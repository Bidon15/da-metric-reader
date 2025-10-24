# Logging Configuration

## Default Output (Clean)

By default, the application runs with **minimal output** to keep the terminal clean. You'll only see:

### Startup Messages

```
🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
📊 Sampler will tick every 60 seconds
📦 Batches will be generated every 3600 seconds
```

### Sample Failures (if any)

```
✗ Sample #1234 - Head: Some(12345), Headers: Some(98765), FAILED: head not advanced (prev: Some(12345), curr: Some(12345))
```

_Note: Successful samples are logged at DEBUG level and won't show by default_

### Batch Generation (every window_secs)

```
================================================================================
📦 BATCH GENERATED - Would post to Celestia DA
================================================================================
🕐 Time Window:
   Start: 1729785600 (2025-10-24 12:00:00 UTC)
   End:   1729789200 (2025-10-24 13:00:00 UTC)

📊 Statistics:
   Total Samples:     60
   Successful (OK):   58
   Failed:            2
   Uptime:            96.67%
   Threshold:         57 (95%)
   Meets Threshold:   ✅ YES

🔐 Cryptographic Data:
   Bitmap Hash:       a1b2c3d4e5f6...
   Bitmap Length:     60 bytes

📄 Files Written:
   - data/batch.json
   - data/bitmap.hex
   - data/samples.json

💾 What would be posted to DA:
{
  "batch": {
    "n": 60,
    "good": 58,
    "threshold": 57,
    "bitmap_hash": "a1b2c3d4e5f6...",
    "window": {
      "start": 1729785600,
      "end": 1729789200
    }
  },
  "namespace": "0x2N1CE",
  "timestamp": 1729789200
}
================================================================================
```

## Debug Mode (Verbose)

To see **all details** including successful samples and metric processing:

```bash
RUST_LOG=debug cargo run
```

or

```bash
RUST_LOG=da_reader=debug cargo run
```

### Additional Debug Output

With debug logging enabled, you'll also see:

#### Every Successful Sample

```
✓ Sample #1234 - Head: Some(12345), Headers: Some(98765), OK: ✅
```

#### OTLP Metric Ingestion

```
Content-Type: application/x-protobuf, Content-Encoding: gzip, Body size: 1234 bytes
Decompressing gzipped body
Decompressed 1234 bytes to 5678 bytes
Successfully decoded protobuf metrics
```

#### Normalized Metrics

```
Received 15 normalized metrics
  das_sampled_chain_head [Gauge] = 12345
  das_total_sampled_headers [Gauge] = 98765
  http.server.duration [Histogram] count=150, sum=45230.50
  ...
```

#### DAS Metric Updates

```
Updated DAS head: 12345
Updated DAS headers: 98765
```

## Log Levels

The application uses standard Rust tracing levels:

| Level   | What it shows                                           |
| ------- | ------------------------------------------------------- |
| `error` | Critical errors only                                    |
| `warn`  | Warnings (e.g., decoding failures, staleness)           |
| `info`  | Important events (startup, batches, sample failures)    |
| `debug` | Detailed information (all samples, metrics, processing) |
| `trace` | Very verbose (not used in this app)                     |

## Custom Log Filtering

You can set different levels for different modules:

```bash
# Only debug for the sampler, info for everything else
RUST_LOG=info,da_reader::run_sampler=debug cargo run

# Debug everything except OTLP decoding
RUST_LOG=debug,da_reader::handle_metrics=info cargo run
```

## Production Recommendations

### Normal Operations

```bash
RUST_LOG=info cargo run
```

Shows startup, batches, and any failures. No clutter from successful samples.

### Debugging DAS Node Issues

```bash
RUST_LOG=debug cargo run
```

Shows every sample tick and metric value to diagnose why samples are failing.

### Investigating Metric Ingestion

```bash
RUST_LOG=da_reader::handle_metrics=debug cargo run
```

Shows detailed OTLP decoding without flooding with sample ticks.

## Redirecting Logs

### To a file

```bash
cargo run 2>&1 | tee logs/output.log
```

### JSON format (using tracing-subscriber)

For structured logging, you could add the `tracing-subscriber` JSON feature and configure it in `main()`.

## What Gets Printed vs Logged

- **Batch generation summary** → Always printed to stdout (not affected by log level)
- **Sample ticks** → DEBUG for success, INFO for failures
- **Metric ingestion** → DEBUG
- **DAS metric updates** → DEBUG
- **Warnings/errors** → Always shown (WARN/ERROR level)

This ensures the batch generation output is always visible regardless of log settings, while routine operations only appear when debugging.
