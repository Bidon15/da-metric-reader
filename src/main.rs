use axum::{routing::post, Router, extract::State, http::StatusCode};
use axum::http::HeaderMap;
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::KeyValue;
use prost::Message;
use std::{collections::HashMap, io::Read, net::SocketAddr, time::{SystemTime, UNIX_EPOCH}};
use tokio::net::TcpListener;
use tracing::{info, warn, debug};
use flate2::read::GzDecoder;
use serde::{Serialize, Deserialize};

#[derive(Clone, Default)]
struct AppState;

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

    let app = Router::new()
        .route("/v1/metrics", post(handle_metrics))
        .with_state(AppState);

    let addr: SocketAddr = "0.0.0.0:4318".parse()?;
    info!("Listening for OTLP/HTTP on http://{addr}");
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

/// Accept OTLP/HTTP metrics (JSON or protobuf) and print them
async fn handle_metrics(
    State(_state): State<AppState>,
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
    
    info!("Content-Type: {}, Content-Encoding: {}, Body size: {} bytes", 
          content_type, content_encoding, body.len());
    
    let is_json = content_type.contains("json");
    
    // Decompress body if gzipped
    let decoded_body = if content_encoding.contains("gzip") {
        info!("Decompressing gzipped body");
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = Vec::new();
        match decoder.read_to_end(&mut decompressed) {
            Ok(size) => {
                info!("Decompressed {} bytes to {} bytes", body.len(), size);
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
                info!("Successfully decoded JSON metrics");
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
                info!("Successfully decoded protobuf metrics");
                Ok(req)
        }
        Err(e) => {
            warn!("Failed to decode OTLP protobuf: {e}");
                // If protobuf fails, try JSON as fallback
                match serde_json::from_slice::<ExportMetricsServiceRequest>(&decoded_body) {
                    Ok(req) => {
                        info!("Successfully decoded JSON metrics (fallback)");
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
        print_normalized_metrics(&normalized);
        
        // Example: Process specific metrics
        process_metrics(&normalized);
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

/// Print normalized metrics in a readable format
fn print_normalized_metrics(metrics: &[NormalizedMetric]) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    println!("\n=== Received {} metrics at {} ===", metrics.len(), now);
    
    for metric in metrics {
        println!("\n📊 Metric: {}", metric.name);
        println!("   Type: {}", metric.metric_type);
        
        match &metric.value {
            MetricValue::Int(i) => println!("   Value: {}", i),
            MetricValue::Double(d) => println!("   Value: {:.2}", d),
            MetricValue::Histogram { count, sum, buckets } => {
                println!("   Count: {}", count);
                if let Some(s) = sum {
                    println!("   Sum: {:.2}", s);
                }
                if !buckets.is_empty() {
                    println!("   Buckets:");
                    for bucket in buckets {
                        println!("     ≤ {:.2}: {} observations", bucket.upper_bound, bucket.count);
                    }
                }
            }
            MetricValue::Summary { count, sum, quantiles } => {
                println!("   Count: {}", count);
                println!("   Sum: {:.2}", sum);
                if !quantiles.is_empty() {
                    println!("   Quantiles:");
                    for q in quantiles {
                        println!("     p{:.2}: {:.2}", q.quantile * 100.0, q.value);
                    }
                }
            }
        }
        
        if !metric.attributes.is_empty() {
            println!("   Labels:");
            for (key, value) in &metric.attributes {
                println!("     {}: {}", key, value);
            }
        }
        
        if !metric.resource_attributes.is_empty() {
            println!("   Resource:");
            for (key, value) in &metric.resource_attributes {
                println!("     {}: {}", key, value);
            }
        }
        
        if let (Some(scope_name), Some(scope_version)) = (&metric.scope_name, &metric.scope_version) {
            if !scope_name.is_empty() {
                println!("   Scope: {} ({})", scope_name, scope_version);
            }
        }
    }
    
    println!("\n✅ You can also export these metrics as JSON:");
    if let Ok(json) = serde_json::to_string_pretty(&metrics) {
        println!("{}", json);
    }
}

/// Example function showing how to process normalized metrics
fn process_metrics(metrics: &[NormalizedMetric]) {
    println!("\n=== Processing Metrics ===");
    
    // Example 1: Group by service name
    let mut metrics_by_service: HashMap<String, Vec<&NormalizedMetric>> = HashMap::new();
    for metric in metrics {
        if let Some(service) = metric.resource_attributes.get("service.name") {
            metrics_by_service
                .entry(service.clone())
                .or_insert_with(Vec::new)
                .push(metric);
        }
    }
    
    for (service, service_metrics) in &metrics_by_service {
        info!("Service '{}' sent {} metrics", service, service_metrics.len());
    }
    
    // Example 2: Calculate statistics for histograms
    for metric in metrics {
        if let MetricValue::Histogram { count, sum, buckets } = &metric.value {
            if let Some(s) = sum {
                let avg = s / (*count as f64);
                info!(
                    "📈 {} - Average: {:.2}, Total samples: {}",
                    metric.name, avg, count
                );
                
                // Calculate approximate percentiles from buckets
                if !buckets.is_empty() {
                    let p95_threshold = ((*count as f64) * 0.95) as u64;
                    let mut cumulative = 0u64;
                    
                    for bucket in buckets {
                        cumulative += bucket.count;
                        if cumulative >= p95_threshold {
                            info!("  → p95: ≤ {:.2}", bucket.upper_bound);
                            break;
                        }
                    }
                }
            }
        }
    }
    
    // Example 3: Alert on high values
    for metric in metrics {
        // Example: Alert on high response times (>1 second)
        if metric.name.contains("duration") || metric.name.contains("latency") {
            match &metric.value {
                MetricValue::Double(val) if *val > 1000.0 => {
                    let labels_str: Vec<String> = metric.attributes
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    warn!(
                        "⚠️  High latency: {:.2}ms for {} [{}]",
                        val, metric.name, labels_str.join(", ")
                    );
                }
                _ => {}
            }
        }
        
        // Example: Alert on error counts (threshold: 50)
        // Adjust this threshold based on your needs
        if metric.name.contains("error") || metric.name.contains("failure") {
            if let MetricValue::Int(count) = metric.value {
                if count > 50 {
                    let labels_str: Vec<String> = metric.attributes
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    warn!(
                        "⚠️  Error count is high: {} for {} [{}]",
                        count, metric.name, labels_str.join(", ")
                    );
                }
            }
        }
    }
    
    // Example 4: Filter metrics by label
    let http_get_metrics: Vec<_> = metrics
        .iter()
        .filter(|m| {
            m.attributes
                .get("http.method")
                .map(|method| method == "GET")
                .unwrap_or(false)
        })
        .collect();
    
    if !http_get_metrics.is_empty() {
        info!("Found {} GET request metrics", http_get_metrics.len());
    }
    
    println!("=== Processing Complete ===\n");
}