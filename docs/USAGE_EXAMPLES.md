# Normalized Metrics Usage Examples

The OTLP metrics are now normalized into a simple `NormalizedMetric` structure that's easy to process. Here are examples of what you can do with the normalized data:

## Structure Overview

Each `NormalizedMetric` contains:

- **name**: Metric name (e.g., "http.server.duration")
- **metric_type**: "Gauge", "Sum", "Histogram", or "Summary"
- **value**: The actual metric value (Int, Double, Histogram, or Summary)
- **attributes**: Labels/tags attached to the metric (e.g., `{"method": "GET", "status": "200"}`)
- **resource_attributes**: Service-level attributes (e.g., `{"service.name": "my-api", "host.name": "server-1"}`)
- **scope_name/scope_version**: Instrumentation library info
- **time_unix_nano**: Timestamp when the metric was recorded
- **start_time_unix_nano**: Start time for cumulative metrics

## Example Use Cases

### 1. Filter Metrics by Name

```rust
// In handle_metrics function, after normalizing:
let http_metrics: Vec<&NormalizedMetric> = normalized
    .iter()
    .filter(|m| m.name.starts_with("http."))
    .collect();

info!("Found {} HTTP metrics", http_metrics.len());
```

### 2. Extract Metrics by Service

```rust
let metrics_by_service: HashMap<String, Vec<&NormalizedMetric>> = normalized
    .iter()
    .filter_map(|m| {
        m.resource_attributes
            .get("service.name")
            .map(|service| (service.clone(), m))
    })
    .fold(HashMap::new(), |mut acc, (service, metric)| {
        acc.entry(service).or_insert_with(Vec::new).push(metric);
        acc
    });

for (service, metrics) in metrics_by_service {
    info!("Service '{}' sent {} metrics", service, metrics.len());
}
```

### 3. Process Different Metric Types

```rust
for metric in &normalized {
    match &metric.value {
        MetricValue::Int(val) => {
            // Process counter or gauge
            info!("{}: {}", metric.name, val);
        }
        MetricValue::Double(val) => {
            // Process floating-point metric
            info!("{}: {:.2}", metric.name, val);
        }
        MetricValue::Histogram { count, sum, buckets } => {
            // Calculate average from histogram
            if let Some(s) = sum {
                let avg = s / (*count as f64);
                info!("{} average: {:.2}ms (from {} samples)",
                      metric.name, avg, count);
            }

            // Find p95 (95th percentile) from buckets
            let p95_threshold = (count as f64 * 0.95) as u64;
            let mut cumulative = 0;
            for bucket in buckets {
                cumulative += bucket.count;
                if cumulative >= p95_threshold {
                    info!("{} p95: ≤ {:.2}ms", metric.name, bucket.upper_bound);
                    break;
                }
            }
        }
        MetricValue::Summary { count, sum, quantiles } => {
            let avg = sum / (*count as f64);
            info!("{} average: {:.2}", metric.name, avg);

            // Find specific quantiles
            for q in quantiles {
                if q.quantile == 0.95 {
                    info!("{} p95: {:.2}", metric.name, q.value);
                }
            }
        }
    }
}
```

### 4. Store in a Time-Series Database

```rust
// Example: Convert to Prometheus remote write format
for metric in &normalized {
    let labels: Vec<_> = metric.attributes
        .iter()
        .chain(metric.resource_attributes.iter())
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect();

    let timestamp_ms = metric.time_unix_nano.unwrap_or(0) / 1_000_000;

    match &metric.value {
        MetricValue::Int(val) => {
            let prom_metric = format!(
                "{}{{{}}} {} {}",
                metric.name,
                labels.join(","),
                val,
                timestamp_ms
            );
            // Send to Prometheus, InfluxDB, etc.
        }
        MetricValue::Double(val) => {
            let prom_metric = format!(
                "{}{{{}}} {} {}",
                metric.name,
                labels.join(","),
                val,
                timestamp_ms
            );
            // Send to database
        }
        // Handle histograms and summaries...
        _ => {}
    }
}
```

### 5. Export as JSON

```rust
// Already built in! The metrics are serializable:
let json = serde_json::to_string_pretty(&normalized)?;
println!("{}", json);

// Or save to file:
std::fs::write("metrics.json", json)?;
```

### 6. Filter by Labels/Attributes

```rust
// Find all metrics with status code 500
let error_metrics: Vec<&NormalizedMetric> = normalized
    .iter()
    .filter(|m| {
        m.attributes
            .get("http.status_code")
            .map(|code| code == "500")
            .unwrap_or(false)
    })
    .collect();

info!("Found {} metrics with status 500", error_metrics.len());
```

### 7. Calculate Aggregates

```rust
use std::collections::HashMap;

// Sum all values by metric name
let mut sums: HashMap<String, f64> = HashMap::new();
let mut counts: HashMap<String, usize> = HashMap::new();

for metric in &normalized {
    let value = match &metric.value {
        MetricValue::Int(v) => *v as f64,
        MetricValue::Double(v) => *v,
        _ => continue, // Skip histograms/summaries
    };

    *sums.entry(metric.name.clone()).or_insert(0.0) += value;
    *counts.entry(metric.name.clone()).or_insert(0) += 1;
}

// Calculate averages
for (name, sum) in sums {
    let count = counts[&name];
    let avg = sum / count as f64;
    info!("{} average: {:.2}", name, avg);
}
```

### 8. Alert on Thresholds

```rust
for metric in &normalized {
    // Alert if response time is too high
    if metric.name == "http.server.duration" {
        if let MetricValue::Double(duration_ms) = metric.value {
            if duration_ms > 1000.0 {
                warn!(
                    "⚠️  High response time: {:.2}ms for {} {}",
                    duration_ms,
                    metric.attributes.get("http.method").unwrap_or(&"?".to_string()),
                    metric.attributes.get("http.route").unwrap_or(&"?".to_string())
                );
            }
        }
    }

    // Alert on high error rates
    if metric.name.contains("error") || metric.name.contains("failure") {
        if let MetricValue::Int(count) = metric.value {
            if count > 10 {
                warn!("⚠️  High error count: {} for {}", count, metric.name);
            }
        }
    }
}
```

## Integration Ideas

1. **Forward to Prometheus**: Use the Prometheus remote write API
2. **Store in InfluxDB**: Convert to Line Protocol format
3. **Send to DataDog**: Use their metrics API
4. **Write to PostgreSQL/TimescaleDB**: Store time-series data
5. **Stream to Kafka**: Publish metrics for downstream processing
6. **Create Custom Dashboards**: Use the JSON output with Grafana/Kibana
7. **Machine Learning**: Feed normalized data into ML models for anomaly detection

## Running the Server

```bash
# Build and run
cargo run

# Test with curl (protobuf - requires actual OTLP data)
curl -X POST http://localhost:4318/v1/metrics \
  -H "Content-Type: application/x-protobuf" \
  --data-binary @metrics.pb

# Or send from an OpenTelemetry SDK
# The normalized output will be printed to stdout and can be processed
```

The normalized metrics make it much easier to:

- Filter and search metrics
- Convert to other formats
- Aggregate and analyze
- Store in databases
- Create alerts and dashboards
