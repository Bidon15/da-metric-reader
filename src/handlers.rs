use axum::{extract::State, http::{StatusCode, HeaderMap}};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use prost::Message;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};
use flate2::read::GzDecoder;
use crate::types::{AppState, NormalizedMetric, MetricValue};
use crate::otlp::{normalize_metrics, print_normalized_metrics};

/// Accept OTLP/HTTP metrics (JSON or protobuf) and extract DAS metrics
pub async fn handle_metrics(
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

