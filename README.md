# OTLP Metrics Reader

A Rust-based OTLP (OpenTelemetry Protocol) HTTP metrics receiver that normalizes incoming metrics into an easy-to-process format.

## Features

✅ **Receives OTLP Metrics** via HTTP (port 4318)  
✅ **Supports Multiple Formats**: Protobuf and JSON  
✅ **Handles Compression**: Automatic gzip decompression  
✅ **Normalizes Metrics**: Converts complex OTLP structures into simple, processable format  
✅ **Extracts All Data**: Resource attributes, labels, timestamps, and values  
✅ **JSON Export**: Serialize normalized metrics to JSON  
✅ **Built-in Examples**: Shows how to filter, aggregate, and alert on metrics

## Quick Start

```bash
# Build the project
cargo build

# Run the server
cargo run

# Server will listen on http://0.0.0.0:4318/v1/metrics
```

## Sending Metrics

The server accepts OTLP metrics from any OpenTelemetry SDK. For example:

### From an OpenTelemetry Application

```javascript
// JavaScript/Node.js example
const {
  MeterProvider,
  PeriodicExportingMetricReader,
} = require("@opentelemetry/sdk-metrics");
const {
  OTLPMetricExporter,
} = require("@opentelemetry/exporter-metrics-otlp-http");

const exporter = new OTLPMetricExporter({
  url: "http://localhost:4318/v1/metrics",
});

const meterProvider = new MeterProvider({
  readers: [new PeriodicExportingMetricReader({ exporter })],
});

const meter = meterProvider.getMeter("example");
const counter = meter.createCounter("http_requests_total");

counter.add(1, { method: "GET", route: "/api/users" });
```

### Manual cURL Test (with JSON)

```bash
curl -X POST http://localhost:4318/v1/metrics \
  -H "Content-Type: application/json" \
  -d '{
    "resourceMetrics": [{
      "resource": {
        "attributes": [{
          "key": "service.name",
          "value": {"stringValue": "test-service"}
        }]
      },
      "scopeMetrics": [{
        "metrics": [{
          "name": "test.counter",
          "sum": {
            "dataPoints": [{
              "asInt": "42",
              "timeUnixNano": "1234567890000000000"
            }]
          }
        }]
      }]
    }]
  }'
```

## Normalized Metric Structure

The server converts OTLP metrics into a simplified `NormalizedMetric` structure:

```rust
struct NormalizedMetric {
    name: String,                              // e.g., "http.server.duration"
    metric_type: String,                       // "Gauge", "Sum", "Histogram", "Summary"
    value: MetricValue,                        // Int, Double, Histogram, or Summary
    attributes: HashMap<String, String>,       // Labels like {"method": "GET", "status": "200"}
    resource_attributes: HashMap<String, String>, // {"service.name": "my-api"}
    scope_name: Option<String>,                // Instrumentation library name
    scope_version: Option<String>,             // Instrumentation library version
    time_unix_nano: Option<u64>,              // Timestamp in nanoseconds
    start_time_unix_nano: Option<u64>,        // Start time for cumulative metrics
}
```

### Metric Value Types

```rust
enum MetricValue {
    Int(i64),                    // Counter or gauge with integer value
    Double(f64),                 // Counter or gauge with floating-point value
    Histogram {                  // Distribution of observations
        count: u64,
        sum: Option<f64>,
        buckets: Vec<HistogramBucket>,
    },
    Summary {                    // Pre-calculated quantiles
        count: u64,
        sum: f64,
        quantiles: Vec<SummaryQuantile>,
    },
}
```

## Processing Examples

The server includes several built-in examples (see `process_metrics()` function):

### 1. Group Metrics by Service

```rust
for metric in metrics {
    if let Some(service) = metric.resource_attributes.get("service.name") {
        println!("Metric {} from service {}", metric.name, service);
    }
}
```

### 2. Calculate Histogram Statistics

```rust
if let MetricValue::Histogram { count, sum, buckets } = &metric.value {
    if let Some(s) = sum {
        let avg = s / (*count as f64);
        println!("Average: {:.2}", avg);

        // Calculate p95
        let p95_threshold = ((*count as f64) * 0.95) as u64;
        let mut cumulative = 0;
        for bucket in buckets {
            cumulative += bucket.count;
            if cumulative >= p95_threshold {
                println!("p95: ≤ {:.2}", bucket.upper_bound);
                break;
            }
        }
    }
}
```

### 3. Alert on Thresholds

```rust
if metric.name.contains("duration") {
    if let MetricValue::Double(val) = metric.value {
        if val > 1000.0 {
            eprintln!("⚠️  High latency: {:.2}ms", val);
        }
    }
}
```

### 4. Filter by Labels

```rust
let get_requests: Vec<_> = metrics
    .iter()
    .filter(|m| m.attributes.get("http.method") == Some(&"GET".to_string()))
    .collect();
```

### 5. Export as JSON

```rust
let json = serde_json::to_string_pretty(&metrics)?;
println!("{}", json);
```

## Output Example

When metrics are received, you'll see output like:

```
=== Received 3 metrics at 1729785600 ===

📊 Metric: http.server.duration
   Type: Histogram
   Count: 150
   Sum: 45230.50
   Buckets:
     ≤ 100.00: 80 observations
     ≤ 500.00: 130 observations
     ≤ 1000.00: 145 observations
     ≤ 5000.00: 150 observations
   Labels:
     http.method: GET
     http.route: /api/users
     http.status_code: 200
   Resource:
     service.name: my-api
     host.name: server-1
   Scope: opentelemetry.instrumentation.http (1.0.0)

📊 Metric: http.server.active_requests
   Type: Gauge
   Value: 12
   Labels:
     http.method: GET
   Resource:
     service.name: my-api
     host.name: server-1

=== Processing Metrics ===
Service 'my-api' sent 3 metrics
📈 http.server.duration - Average: 301.54, Total samples: 150
  → p95: ≤ 1000.00
=== Processing Complete ===
```

## Use Cases

1. **Development/Testing**: Local OTLP endpoint for testing OpenTelemetry instrumentation
2. **Metrics Gateway**: Receive metrics from multiple services and forward to databases
3. **Monitoring**: Alert on specific metric patterns or thresholds
4. **Data Processing**: Transform and enrich metrics before storage
5. **Analytics**: Calculate custom statistics and aggregations
6. **Debugging**: Inspect raw OTLP metrics in a readable format

## Integration Ideas

- **Forward to Prometheus**: Convert and send via remote write API
- **Store in TimescaleDB**: Insert normalized metrics as time-series data
- **Stream to Kafka**: Publish metrics for downstream processing
- **Send to DataDog/New Relic**: Convert format and forward
- **Custom Dashboards**: Export JSON for visualization tools
- **Machine Learning**: Feed normalized data into ML pipelines

## Dependencies

- `axum` - Web framework
- `tokio` - Async runtime
- `prost` - Protobuf serialization
- `opentelemetry-proto` - OTLP protocol definitions
- `serde` & `serde_json` - JSON serialization
- `flate2` - Gzip decompression
- `tracing` - Logging

## Architecture

```
┌─────────────┐      OTLP/HTTP       ┌──────────────┐
│ OpenTelemetry│ ─────Protobuf/JSON──▶│              │
│   SDK        │      (gzipped)       │  da-reader   │
└─────────────┘                       │              │
                                      │  1. Decode   │
┌─────────────┐                       │  2. Normalize│
│ Application │ ─────────────────────▶│  3. Process  │
└─────────────┘      Port 4318        │  4. Export   │
                                      └──────────────┘
                                            │
                                            ▼
                               ┌─────────────────────────┐
                               │ • Print to stdout       │
                               │ • Export as JSON        │
                               │ • Forward to DB         │
                               │ • Alert on thresholds   │
                               │ • Calculate statistics  │
                               └─────────────────────────┘
```

## Further Reading

- See `USAGE_EXAMPLES.md` for more code examples
- [OpenTelemetry Protocol Spec](https://github.com/open-telemetry/opentelemetry-proto)
- [OTLP Metrics](https://opentelemetry.io/docs/specs/otlp/#otlphttp-request)

## License

MIT
