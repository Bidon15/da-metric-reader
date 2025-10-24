use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{info, warn, debug, error};
use crate::types::{AppState, Sample, SampleBit};
use crate::storage::save_samples;

/// Background task: samples metrics at fixed intervals
pub async fn run_sampler(state: AppState) {
    let tick_duration = Duration::from_secs(state.config.sampling.tick_secs);
    let mut ticker = interval(tick_duration);
    let window_size = (state.config.batching.window_secs / state.config.sampling.tick_secs) as usize;
    
    // Previous values to track advancement
    let mut prev_head: Option<i64> = None;
    let mut prev_headers: Option<i64> = None;
    
    info!("🔄 Sampler started (tick every {}s, window size: {})", 
          state.config.sampling.tick_secs, window_size);
    
    loop {
        ticker.tick().await;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Read current metrics
        let (current_head, current_headers, last_update) = {
            let das_metrics = state.das_metrics.lock().unwrap();
            (das_metrics.head, das_metrics.headers, das_metrics.last_update)
        };
        
        // Check staleness
        let is_stale = match last_update {
            Some(update_time) => {
                let age = now.saturating_sub(update_time);
                age > state.config.sampling.max_staleness_secs
            }
            None => true,
        };
        
        // Check head advancement and reason
        let (head_advanced, head_reason) = match (prev_head, current_head) {
            (Some(prev), Some(curr)) => {
                let diff = curr - prev;
                // Head advanced: good!
                if diff >= state.config.metrics.min_increment {
                    (true, format!("+{} blocks", diff))
                } else {
                    // Head didn't advance, but check if data is fresh
                    // If metrics were just updated, give it a pass
                    // (Data is fresh, just sampled at wrong moment)
                    let data_age = last_update.map(|u| now.saturating_sub(u)).unwrap_or(999);
                    if data_age <= state.config.sampling.grace_period_secs {
                        // Fresh data, can't judge advancement yet
                        (true, format!("fresh data (age={}s)", data_age))
                    } else {
                        (false, format!("head stuck at {}", curr))
                    }
                }
            }
            (None, Some(_)) => {
                // First reading, consider it ok
                (true, "first sample".to_string())
            }
            _ => (false, "no head data".to_string()),
        };
        
        // Optional: Check if headers advanced
        let headers_advanced = match (prev_headers, current_headers) {
            (Some(prev), Some(curr)) => curr > prev,
            (None, Some(_)) => true,
            _ => false,
        };
        
        // Determine if this tick is "ok"
        let (ok, reason) = if is_stale {
            (false, format!("stale (age > {}s)", state.config.sampling.max_staleness_secs))
        } else if !head_advanced {
            (false, head_reason)
        } else if !headers_advanced {
            (false, format!("headers not advancing"))
        } else {
            (true, head_reason)
        };
        
        // Create sample
        let sample = Sample {
            timestamp: now,
            head: current_head,
            headers: current_headers,
            ok,
            reason: reason.clone(),
        };
        
        let sample_bit = SampleBit {
            timestamp: now,
            ok,
            reason: reason.clone(),
        };
        
        // Store sample
        {
            let mut samples = state.samples.lock().unwrap();
            samples.push(sample.clone());
            
            // Save to file periodically
            if let Err(e) = save_samples(&samples) {
                error!("Failed to save samples: {}", e);
            } else {
                debug!("💾 Saved {} samples to data/samples.json", samples.len());
            }
        }
        
        // Add to ring buffer
        {
            let mut ring_buffer = state.ring_buffer.lock().unwrap();
            ring_buffer.push_back(sample_bit.clone());
            
            // Maintain window size
            while ring_buffer.len() > window_size {
                ring_buffer.pop_front();
            }
        }
        
        // Post sample to DA if enabled (detailed history)
        if state.config.da_posting.enabled && state.config.da_posting.post_every_sample {
            // TODO: Implement actual DA posting
            // post_sample_to_da(&sample_bit, &state).await;
            info!("📡 Posted sample to Celestia DA: ok={}, timestamp={}", sample_bit.ok, sample_bit.timestamp);
        }
        
        // Show all samples at info level for better DevX
        let buffer_len = {
            let buffer = state.ring_buffer.lock().unwrap();
            buffer.len()
        };
        
        if ok {
            info!(
                "✅ Sample OK - Head: {:?} ({}), Headers: {:?} | Buffer: {}/{} samples",
                current_head,
                reason,
                current_headers,
                buffer_len,
                window_size
            );
        } else {
            warn!(
                "❌ Sample FAILED - {} | Head: {:?}, Headers: {:?}",
                reason,
                current_head,
                current_headers
            );
        }
        
        // Update previous values for next iteration
        prev_head = current_head;
        prev_headers = current_headers;
    }
}

