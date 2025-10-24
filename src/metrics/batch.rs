use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{info, warn, error};
use crate::types::{AppState, Batch, TimeWindow, SampleBit};
use crate::storage::{save_batch, save_bitmap};
use crate::utils::format_timestamp;

/// Background task: generates batches at fixed intervals (for ZK proofs)
pub async fn run_batch_generator(state: AppState) {
    let batch_duration = Duration::from_secs(state.config.batching.window_secs);
    let mut ticker = interval(batch_duration);
    
    info!("📦 Batch generator started (every {}s = {} min) for ZK proof generation", 
          state.config.batching.window_secs,
          state.config.batching.window_secs / 60);
    
    // Skip the first immediate tick
    ticker.tick().await;
    
    loop {
        ticker.tick().await;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Get the ring buffer
        let bits: Vec<SampleBit> = {
            let ring_buffer = state.ring_buffer.lock().unwrap();
            ring_buffer.iter().cloned().collect()
        };
        
        if bits.is_empty() {
            warn!("No samples in ring buffer yet, skipping batch");
            continue;
        }
        
        // Generate batch
        let n = bits.len();
        let good = bits.iter().filter(|b| b.ok).count();
        let threshold = ((n as f64) * state.config.proofs.threshold_percent).ceil() as usize;
        
        let window_start = bits.first().map(|b| b.timestamp).unwrap_or(now);
        let window_end = bits.last().map(|b| b.timestamp).unwrap_or(now);
        
        // Create bitmap (1 = ok, 0 = not ok)
        let bitmap_bytes: Vec<u8> = bits.iter().map(|b| if b.ok { 1 } else { 0 }).collect();
        
        // Hash the bitmap
        let bitmap_hash = blake3::hash(&bitmap_bytes);
        let bitmap_hash_hex = bitmap_hash.to_hex();
        
        // Create batch
        let batch = Batch {
            n,
            good,
            threshold,
            bitmap_hash: bitmap_hash_hex.to_string(),
            window: TimeWindow {
                start: window_start,
                end: window_end,
            },
        };
        
        // Save batch
        if let Err(e) = save_batch(&batch) {
            error!("Failed to save batch: {}", e);
        }
        
        // Save bitmap
        if let Err(e) = save_bitmap(&bitmap_bytes) {
            error!("Failed to save bitmap: {}", e);
        }
        
        // Print what would be posted to DA
        print_batch_summary(&batch, &bitmap_bytes, &state, now);
        
        let uptime_percent = (good as f64 / n as f64) * 100.0;
        let meets_threshold = good >= threshold;
        
        info!(
            "✅ Batch generated: n={}, good={}, threshold={}, uptime={:.2}%",
            n, good, threshold, uptime_percent
        );
        
        if meets_threshold {
            info!("🎉 Uptime threshold MET ({:.0}%) - Batch ready for ZK proof generation", 
                  state.config.proofs.threshold_percent * 100.0);
        } else {
            warn!("⚠️  Uptime threshold NOT MET - ZK proof would fail (need {:.0}%, got {:.2}%)", 
                  state.config.proofs.threshold_percent * 100.0,
                  uptime_percent);
        }
        
        info!("💾 Batch files saved to data/ directory (batch.json, bitmap.hex)");
        
        // TODO: Generate ZK proof
        info!("🔐 TODO: Generate ZK proof from this batch");
        // let proof = generate_zk_proof(&batch, &bitmap_bytes).await;
        
        // Post batch + proof to DA (verifiable attestation)
        if state.config.da_posting.enabled {
            info!("✅ Individual samples already posted to DA (detailed history)");
            info!("📡 TODO: Post batch summary + ZK proof to DA (verifiable attestation)");
            // TODO: Implement batch posting to DA
            // post_batch_to_da(&batch, &proof, &state).await;
        } else {
            info!("📡 DA posting disabled - samples and batches stored locally only");
        }
    }
}

/// Print batch summary for visual clarity
fn print_batch_summary(batch: &Batch, bitmap_bytes: &[u8], state: &AppState, now: u64) {
    let uptime_percent = (batch.good as f64 / batch.n as f64) * 100.0;
    let meets_threshold = batch.good >= batch.threshold;
    
    println!("\n{}", "=".repeat(80));
    println!("📦 BATCH GENERATED FOR ZK PROOF");
    println!("   This batch is for generating ZK proofs of uptime");
    println!("   (Individual samples are posted to DA separately)");
    println!("{}", "=".repeat(80));
    println!("🕐 Time Window:");
    println!("   Start: {} ({})", batch.window.start, format_timestamp(batch.window.start));
    println!("   End:   {} ({})", batch.window.end, format_timestamp(batch.window.end));
    println!("\n📊 Statistics:");
    println!("   Total Samples:     {}", batch.n);
    println!("   Successful (OK):   {}", batch.good);
    println!("   Failed:            {}", batch.n - batch.good);
    println!("   Uptime:            {:.2}%", uptime_percent);
    println!("   Threshold:         {} ({:.0}%)", batch.threshold, state.config.proofs.threshold_percent * 100.0);
    println!("   Meets Threshold:   {} {}", 
             if meets_threshold { "✅ YES" } else { "❌ NO" },
             if meets_threshold { "" } else { "(Would not generate proof)" });
    println!("\n🔐 Cryptographic Data:");
    println!("   Bitmap Hash:       {}", batch.bitmap_hash);
    println!("   Bitmap Length:     {} bytes", bitmap_bytes.len());
    println!("\n📄 Files Written:");
    println!("   - data/batch.json");
    println!("   - data/bitmap.hex");
    println!("   - data/samples.json");
    println!("\n💾 What would be posted to DA:");
    
    let da_payload = serde_json::json!({
        "batch": {
            "n": batch.n,
            "good": batch.good,
            "threshold": batch.threshold,
            "bitmap_hash": batch.bitmap_hash,
            "window": {
                "start": batch.window.start,
                "end": batch.window.end,
            }
        },
        "namespace": state.config.celestia.namespace,
        "timestamp": now,
    });
    
    println!("{}", serde_json::to_string_pretty(&da_payload).unwrap());
    println!("{}\n", "=".repeat(80));
}

