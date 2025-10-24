mod handlers;

pub use handlers::handle_metrics;

use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::common::v1::KeyValue;
use std::collections::HashMap;
use tracing::debug;
use crate::types::{NormalizedMetric, MetricValue, HistogramBucket, SummaryQuantile};

/// Helper function to convert OTLP KeyValue attributes to HashMap
pub fn attributes_to_map(attributes: Vec<KeyValue>) -> HashMap<String, String> {
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
pub fn normalize_metrics(req: ExportMetricsServiceRequest) -> Vec<NormalizedMetric> {
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
pub fn print_normalized_metrics(metrics: &[NormalizedMetric]) {
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

