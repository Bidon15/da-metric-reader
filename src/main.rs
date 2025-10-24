use axum::{routing::post, Router, extract::State, http::StatusCode};
use axum::http::HeaderMap;
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::KeyValue;
use prost::Message;
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::Read,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, time::interval};
use tracing::{info, warn, debug, error};
use flate2::read::GzDecoder;
use serde::{Serialize, Deserialize};

/// Configuration loaded from config.toml
#[derive(Debug, Clone, Deserialize)]
struct Config {
    sampling: SamplingConfig,
    metrics: MetricsConfig,
    da_posting: DaPostingConfig,
    batching: BatchingConfig,
    #[allow(dead_code)] // Will be used for DA posting later
    celestia: CelestiaConfig,
    proofs: ProofsConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct SamplingConfig {
    tick_secs: u64,
    max_staleness_secs: u64,
    grace_period_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct DaPostingConfig {
    enabled: bool,
    post_every_sample: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct BatchingConfig {
    window_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricsConfig {
    head_metric: String,
    headers_metric: String,
    min_increment: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Will be used for DA posting later
struct CelestiaConfig {
    node_url: String,
    namespace: String,
    poster_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ProofsConfig {
    #[allow(dead_code)] // Will be used for ZK proofs later
    enabled: bool,
    threshold_percent: f64,
}

impl Config {
    fn load() -> anyhow::Result<Self> {
        let content = fs::read_to_string("config.toml")?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Stores the latest DAS metrics
#[derive(Debug, Clone, Default)]
struct DasMetrics {
    head: Option<i64>,
    headers: Option<i64>,
    last_update: Option<u64>, // Unix timestamp in seconds
}

/// Application state shared across handlers and background tasks
#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    das_metrics: Arc<Mutex<DasMetrics>>,
    ring_buffer: Arc<Mutex<VecDeque<SampleBit>>>,
    samples: Arc<Mutex<Vec<Sample>>>,
}

/// A single sample bit with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SampleBit {
    timestamp: u64,
    ok: bool,
    reason: String,
}

/// Raw sample data point
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Sample {
    timestamp: u64,
    head: Option<i64>,
    headers: Option<i64>,
    ok: bool,
    reason: String,
}

/// Normalized metric structure for easier processing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NormalizedMetric {
    /// Metric name (e.g., "http.server.duration")
    name: String,
    /// Type of metric (Sum, Gauge, Histogram, etc.)
    metric_type: String,
    /// The actual metric value
    value: MetricValue,
    /// Labels/attributes attached to this data point
    attributes: HashMap<String, String>,
    /// Resource attributes (service.name, host.name, etc.)
    resource_attributes: HashMap<String, String>,
    /// Instrumentation scope (library name and version)
    scope_name: Option<String>,
    scope_version: Option<String>,
    /// Timestamps in nanoseconds since Unix epoch
    time_unix_nano: Option<u64>,
    start_time_unix_nano: Option<u64>,
}

/// Enum to represent different metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum MetricValue {
    Int(i64),
    Double(f64),
    Histogram {
        count: u64,
        sum: Option<f64>,
        buckets: Vec<HistogramBucket>,
    },
    Summary {
        count: u64,
        sum: f64,
        quantiles: Vec<SummaryQuantile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistogramBucket {
    count: u64,
    upper_bound: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SummaryQuantile {
    quantile: f64,
    value: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Arc::new(Config::load()?);
    info!("Loaded config: {:?}", config);
    
    // Create data directory if it doesn't exist
    fs::create_dir_all("data")?;
    
    // Initialize shared state
    let state = AppState {
        config: config.clone(),
        das_metrics: Arc::new(Mutex::new(DasMetrics::default())),
        ring_buffer: Arc::new(Mutex::new(VecDeque::new())),
        samples: Arc::new(Mutex::new(Vec::new())),
    };
    
    // Spawn background sampler task
    let sampler_state = state.clone();
    tokio::spawn(async move {
        run_sampler(sampler_state).await;
    });
    
    // Spawn background batch generator task
    let batch_state = state.clone();
    tokio::spawn(async move {
        run_batch_generator(batch_state).await;
    });
    
    // Start HTTP server
    let app = Router::new()
        .route("/v1/metrics", post(handle_metrics))
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:4318".parse()?;
    info!("🚀 Listening for OTLP/HTTP on http://{addr}");
    info!("📊 Sampler will tick every {} seconds", config.sampling.tick_secs);
    
    if config.da_posting.enabled {
        if config.da_posting.post_every_sample {
            info!("📡 DA posting: ENABLED - Will post each sample to Celestia DA");
        } else {
            info!("📡 DA posting: ENABLED - Will post batched samples to Celestia DA");
        }
    } else {
        info!("📡 DA posting: DISABLED - Samples will be stored locally only");
    }
    
    info!("📦 Batches (for ZK proofs) will be generated every {} seconds ({} minutes)", 
          config.batching.window_secs, 
          config.batching.window_secs / 60);
    
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

/// Accept OTLP/HTTP metrics (JSON or protobuf) and extract DAS metrics
async fn handle_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> (StatusCode, axum::body::Bytes) {
    // Log incoming request details
    debug!("Received request with {} bytes", body.len());
    
    // Check Content-Type to determine format
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    let content_encoding = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    debug!("Content-Type: {}, Content-Encoding: {}, Body size: {} bytes", 
           content_type, content_encoding, body.len());
    
    let is_json = content_type.contains("json");
    
    // Decompress body if gzipped
    let decoded_body = if content_encoding.contains("gzip") {
        debug!("Decompressing gzipped body");
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = Vec::new();
        match decoder.read_to_end(&mut decompressed) {
            Ok(size) => {
                debug!("Decompressed {} bytes to {} bytes", body.len(), size);
                axum::body::Bytes::from(decompressed)
            }
            Err(e) => {
                warn!("Failed to decompress gzip: {e}");
                return (StatusCode::BAD_REQUEST, axum::body::Bytes::from("Failed to decompress"));
            }
        }
    } else {
        body
    };
    
    // Try to decode based on content type
    let result = if is_json {
        // Try JSON decoding
        match serde_json::from_slice::<ExportMetricsServiceRequest>(&decoded_body) {
        Ok(req) => {
                debug!("Successfully decoded JSON metrics");
                Ok(req)
            }
            Err(e) => {
                warn!("Failed to decode OTLP JSON: {e}");
                debug!("Body preview: {:?}", String::from_utf8_lossy(&decoded_body[..decoded_body.len().min(200)]));
                Err(())
            }
        }
    } else {
        // Try protobuf decoding
        match ExportMetricsServiceRequest::decode(decoded_body.clone()) {
            Ok(req) => {
                debug!("Successfully decoded protobuf metrics");
                Ok(req)
        }
        Err(e) => {
            warn!("Failed to decode OTLP protobuf: {e}");
                // If protobuf fails, try JSON as fallback
                match serde_json::from_slice::<ExportMetricsServiceRequest>(&decoded_body) {
                    Ok(req) => {
                        debug!("Successfully decoded JSON metrics (fallback)");
                        Ok(req)
                    }
                    Err(e2) => {
                        warn!("Failed to decode as JSON too: {e2}");
                        debug!("Body preview: {:?}", String::from_utf8_lossy(&decoded_body[..decoded_body.len().min(200)]));
                        Err(())
                    }
                }
            }
        }
    };
    
    if let Ok(req) = result {
        let normalized = normalize_metrics(req);
        
        // Extract DAS-specific metrics and store them
        let das_updated = extract_das_metrics(&normalized, &state);
        
        // Log successful metric ingestion
        if das_updated {
            info!("📥 Received OTLP metrics from DAS node - Stored internally");
        } else {
            debug!("📥 Received {} OTLP metrics (no DAS-specific metrics found)", normalized.len());
        }
        
        // Only print detailed metrics in debug mode
        if tracing::enabled!(tracing::Level::DEBUG) {
            print_normalized_metrics(&normalized);
        }
    }

    // Reply with appropriate response format
    let resp = ExportMetricsServiceResponse { partial_success: None };
    if is_json {
        let json = serde_json::to_vec(&resp).unwrap();
        (StatusCode::OK, axum::body::Bytes::from(json))
    } else {
    let mut buf = Vec::new();
    prost::Message::encode(&resp, &mut buf).unwrap();
    (StatusCode::OK, axum::body::Bytes::from(buf))
}
}

/// Helper function to convert OTLP KeyValue attributes to HashMap
fn attributes_to_map(attributes: Vec<KeyValue>) -> HashMap<String, String> {
    attributes
        .into_iter()
        .filter_map(|kv| {
            kv.value.and_then(|v| {
                let value_str = match v.value {
                    Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) => Some(s),
                    Some(opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i)) => Some(i.to_string()),
                    Some(opentelemetry_proto::tonic::common::v1::any_value::Value::DoubleValue(d)) => Some(d.to_string()),
                    Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BoolValue(b)) => Some(b.to_string()),
                    _ => None,
                };
                value_str.map(|v| (kv.key, v))
            })
        })
        .collect()
}

/// Normalize OTLP metrics into a simpler, more processable structure
fn normalize_metrics(req: ExportMetricsServiceRequest) -> Vec<NormalizedMetric> {
    let mut normalized_metrics = Vec::new();

    for resource_metric in req.resource_metrics {
        // Extract resource attributes (service name, host, etc.)
        let resource_attrs = resource_metric
            .resource
            .map(|r| attributes_to_map(r.attributes))
            .unwrap_or_default();

        for scope_metric in resource_metric.scope_metrics {
            // Extract scope information
            let (scope_name, scope_version) = scope_metric
                .scope
                .map(|s| (Some(s.name), Some(s.version)))
                .unwrap_or((None, None));

            for metric in scope_metric.metrics {
                let metric_name = metric.name.clone();

                if let Some(data) = metric.data {
                    use opentelemetry_proto::tonic::metrics::v1::metric::Data;
                    
                    match data {
                        Data::Gauge(gauge) => {
                            for dp in gauge.data_points {
                                if let Some(value) = extract_number_value(&dp.value) {
                                    normalized_metrics.push(NormalizedMetric {
                                        name: metric_name.clone(),
                                        metric_type: "Gauge".to_string(),
                                        value,
                                        attributes: attributes_to_map(dp.attributes),
                                        resource_attributes: resource_attrs.clone(),
                                        scope_name: scope_name.clone(),
                                        scope_version: scope_version.clone(),
                                        time_unix_nano: Some(dp.time_unix_nano),
                                        start_time_unix_nano: Some(dp.start_time_unix_nano),
                                    });
                                }
                            }
                        }
                        Data::Sum(sum) => {
                            for dp in sum.data_points {
                                if let Some(value) = extract_number_value(&dp.value) {
                                    normalized_metrics.push(NormalizedMetric {
                                        name: metric_name.clone(),
                                        metric_type: "Sum".to_string(),
                                        value,
                                        attributes: attributes_to_map(dp.attributes),
                                        resource_attributes: resource_attrs.clone(),
                                        scope_name: scope_name.clone(),
                                        scope_version: scope_version.clone(),
                                        time_unix_nano: Some(dp.time_unix_nano),
                                        start_time_unix_nano: Some(dp.start_time_unix_nano),
                                    });
                                }
                            }
                        }
                        Data::Histogram(histogram) => {
                            for dp in histogram.data_points {
                                let buckets = dp
                                    .bucket_counts
                                    .iter()
                                    .zip(dp.explicit_bounds.iter())
                                    .map(|(count, bound)| HistogramBucket {
                                        count: *count,
                                        upper_bound: *bound,
                                    })
                                    .collect();

                                normalized_metrics.push(NormalizedMetric {
                                    name: metric_name.clone(),
                                    metric_type: "Histogram".to_string(),
                                    value: MetricValue::Histogram {
                                        count: dp.count,
                                        sum: dp.sum,
                                        buckets,
                                    },
                                    attributes: attributes_to_map(dp.attributes),
                                    resource_attributes: resource_attrs.clone(),
                                    scope_name: scope_name.clone(),
                                    scope_version: scope_version.clone(),
                                    time_unix_nano: Some(dp.time_unix_nano),
                                    start_time_unix_nano: Some(dp.start_time_unix_nano),
                                });
                            }
                        }
                        Data::Summary(summary) => {
                            for dp in summary.data_points {
                                let quantiles = dp
                                    .quantile_values
                                    .iter()
                                    .map(|qv| SummaryQuantile {
                                        quantile: qv.quantile,
                                        value: qv.value,
                                    })
                                    .collect();

                                normalized_metrics.push(NormalizedMetric {
                                    name: metric_name.clone(),
                                    metric_type: "Summary".to_string(),
                                    value: MetricValue::Summary {
                                        count: dp.count,
                                        sum: dp.sum,
                                        quantiles,
                                    },
                                    attributes: attributes_to_map(dp.attributes),
                                    resource_attributes: resource_attrs.clone(),
                                    scope_name: scope_name.clone(),
                                    scope_version: scope_version.clone(),
                                    time_unix_nano: Some(dp.time_unix_nano),
                                    start_time_unix_nano: Some(dp.start_time_unix_nano),
                                });
                            }
                        }
                        Data::ExponentialHistogram(_) => {
                            // ExponentialHistogram is less common, you can implement if needed
                            debug!("ExponentialHistogram not yet implemented for {}", metric_name);
                        }
                    }
                }
            }
        }
    }

    normalized_metrics
}

/// Extract numeric value from OTLP NumberDataPoint value
fn extract_number_value(
    value: &Option<opentelemetry_proto::tonic::metrics::v1::number_data_point::Value>,
) -> Option<MetricValue> {
    use opentelemetry_proto::tonic::metrics::v1::number_data_point::Value;
    
    value.as_ref().and_then(|v| match v {
        Value::AsInt(i) => Some(MetricValue::Int(*i)),
        Value::AsDouble(d) => Some(MetricValue::Double(*d)),
    })
}

/// Print normalized metrics in a readable format (debug mode only)
fn print_normalized_metrics(metrics: &[NormalizedMetric]) {
    debug!("Received {} normalized metrics", metrics.len());
    
    for metric in metrics {
        match &metric.value {
            MetricValue::Int(i) => {
                debug!("  {} [{}] = {}", metric.name, metric.metric_type, i);
            }
            MetricValue::Double(d) => {
                debug!("  {} [{}] = {:.2}", metric.name, metric.metric_type, d);
            }
            MetricValue::Histogram { count, sum, .. } => {
                if let Some(s) = sum {
                    debug!("  {} [Histogram] count={}, sum={:.2}", metric.name, count, s);
                } else {
                    debug!("  {} [Histogram] count={}", metric.name, count);
                }
            }
            MetricValue::Summary { count, sum, .. } => {
                debug!("  {} [Summary] count={}, sum={:.2}", metric.name, count, sum);
            }
        }
    }
}


/// Extract DAS-specific metrics and update state
/// Returns true if any DAS metrics were updated
fn extract_das_metrics(metrics: &[NormalizedMetric], state: &AppState) -> bool {
    let config = &state.config.metrics;
    let mut das_metrics = state.das_metrics.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut updated = false;
    
    for metric in metrics {
        // Extract das_sampled_chain_head
        if metric.name == config.head_metric {
            if let MetricValue::Int(value) = metric.value {
                das_metrics.head = Some(value);
                das_metrics.last_update = Some(now);
                debug!("Updated DAS head: {}", value);
                updated = true;
            }
        }
        
        // Extract das_total_sampled_headers
        if metric.name == config.headers_metric {
            if let MetricValue::Int(value) = metric.value {
                das_metrics.headers = Some(value);
                debug!("Updated DAS headers: {}", value);
                updated = true;
            }
        }
    }
    
    updated
}

/// Background task: samples metrics at fixed intervals
async fn run_sampler(state: AppState) {
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

/// Background task: generates batches at fixed intervals (for ZK proofs)
async fn run_batch_generator(state: AppState) {
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
        let uptime_percent = (good as f64 / n as f64) * 100.0;
        let meets_threshold = good >= threshold;
        
        println!("\n{}", "=".repeat(80));
        println!("📦 BATCH GENERATED FOR ZK PROOF");
        println!("   This batch is for generating ZK proofs of uptime");
        println!("   (Individual samples are posted to DA separately)");
        println!("{}", "=".repeat(80));
        println!("🕐 Time Window:");
        println!("   Start: {} ({})", window_start, format_timestamp(window_start));
        println!("   End:   {} ({})", window_end, format_timestamp(window_end));
        println!("\n📊 Statistics:");
        println!("   Total Samples:     {}", n);
        println!("   Successful (OK):   {}", good);
        println!("   Failed:            {}", n - good);
        println!("   Uptime:            {:.2}%", uptime_percent);
        println!("   Threshold:         {} ({:.0}%)", threshold, state.config.proofs.threshold_percent * 100.0);
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
                "n": n,
                "good": good,
                "threshold": threshold,
                "bitmap_hash": batch.bitmap_hash,
                "window": {
                    "start": window_start,
                    "end": window_end,
                }
            },
            "namespace": state.config.celestia.namespace,
            "timestamp": now,
        });
        
        println!("{}", serde_json::to_string_pretty(&da_payload).unwrap());
        println!("{}\n", "=".repeat(80));
        
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

/// Batch structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Batch {
    n: usize,
    good: usize,
    threshold: usize,
    bitmap_hash: String,
    window: TimeWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeWindow {
    start: u64,
    end: u64,
}

/// Save samples to file
fn save_samples(samples: &[Sample]) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(samples)?;
    fs::write("data/samples.json", json)?;
    Ok(())
}

/// Save batch to file
fn save_batch(batch: &Batch) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(batch)?;
    fs::write("data/batch.json", json)?;
    Ok(())
}

/// Save bitmap to hex file
fn save_bitmap(bitmap: &[u8]) -> anyhow::Result<()> {
    let hex: String = bitmap.iter().map(|b| format!("{:02x}", b)).collect();
    fs::write("data/bitmap.hex", hex)?;
    Ok(())
}

/// Format Unix timestamp to human-readable string
fn format_timestamp(ts: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}