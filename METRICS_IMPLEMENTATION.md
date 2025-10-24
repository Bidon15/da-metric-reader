# DAS Metrics Implementation Summary

## ✅ What We've Built

We've implemented the **metrics business logic** from the PRD, specifically sections 5.1, 5.2, and 5.3:

### 1. **Metric Ingestion** (Section 5.1)

- ✅ OTLP/HTTP endpoint at `/v1/metrics`
- ✅ Extracts `das_sampled_chain_head` and `das_total_sampled_headers` metrics
- ✅ Stores latest values with timestamps
- ✅ Supports both JSON and Protobuf encoding
- ✅ Handles gzip compression

### 2. **Sampler** (Section 5.2)

- ✅ Ticks every `tick_secs` (default: 60s)
- ✅ Generates a single bit (0 or 1) per tick
- ✅ Predicate: `ok = head_advanced_by >= 1 AND points_fresh <= max_staleness_secs`
- ✅ Cross-checks that `total_sampled_headers` also advances
- ✅ Maintains a ring buffer with configurable window size (default: 288)
- ✅ Saves samples to `data/samples.json`

### 3. **Batch Generator** (Section 5.3)

- ✅ Runs every `window_secs` (default: 3600s / 1 hour)
- ✅ Computes `good = Σ bits`, `n = len(bits)`
- ✅ Computes `threshold = ceil(0.95 * n)`
- ✅ Hashes bitmap using `blake3(bitmap)`
- ✅ Writes `data/batch.json` and `data/bitmap.hex`
- ✅ Prints what would be posted to DA (instead of actually posting)

## 📁 File Structure

```
da-reader/
├── config.toml                    # Configuration file
├── Cargo.toml                     # Rust dependencies
├── src/
│   └── main.rs                    # Main application
└── data/                          # Generated at runtime
    ├── samples.json               # Raw metric samples
    ├── bitmap.hex                 # Hex-encoded bitmap
    └── batch.json                 # Batch metadata
```

## ⚙️ Configuration (`config.toml`)

```toml
[sampling]
tick_secs = 60              # Sample every 60 seconds
window_secs = 3600          # Generate batch every hour
max_staleness_secs = 120    # Max age for metrics to be considered fresh

[metrics]
head_metric = "das_sampled_chain_head"
headers_metric = "das_total_sampled_headers"
min_increment = 1           # Minimum head advancement per tick

[celestia]
node_url = "http://localhost:26658"
namespace = "0x2N1CE"
poster_mode = "mock"

[proofs]
enabled = false
threshold_percent = 0.95    # 95% uptime threshold
```

## 🚀 How to Run

### 1. Start the Server

```bash
cargo run
```

You'll see:

```
🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
📊 Sampler will tick every 60 seconds
📦 Batches will be generated every 3600 seconds
```

### 2. Send Test Metrics

The server expects OTLP metrics with the following metric names:

- `das_sampled_chain_head` (Gauge) - latest DA block height
- `das_total_sampled_headers` (Gauge) - cumulative headers sampled

Example using your DAS node's OpenTelemetry exporter pointing to `http://localhost:4318/v1/metrics`.

### 3. Watch the Sampler

Every 60 seconds (by default), you'll see:

```
✓ Sample #1234 - Head: Some(12345), Headers: Some(98765), OK: ✅ (ok)
```

or if something is wrong:

```
✓ Sample #5678 - Head: Some(12345), Headers: Some(98765), OK: ❌ (head not advanced)
```

### 4. Wait for Batch Generation

After the configured window (default 1 hour), you'll see a detailed batch summary:

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

## 📊 Business Logic Details

### Sampling Logic

Each tick evaluates:

1. **Staleness Check**: Is `last_update` within `max_staleness_secs`?
2. **Head Advancement**: Has `das_sampled_chain_head` increased by at least `min_increment`?
3. **Headers Advancement**: Has `das_total_sampled_headers` increased?

If all checks pass → `ok = 1` (✅)
If any check fails → `ok = 0` (❌)

### Batch Generation Logic

Every window period:

1. Collects all bits from the ring buffer
2. Counts `good` (number of 1s) and `n` (total bits)
3. Calculates `threshold = ceil(threshold_percent * n)` (default 95%)
4. Creates bitmap as sequence of 0s and 1s
5. Hashes bitmap with BLAKE3
6. Saves to files and prints DA payload

## 📝 Example Output Files

### `data/samples.json`

```json
[
  {
    "timestamp": 1729785600,
    "head": 12345,
    "headers": 98765,
    "ok": true,
    "reason": "ok"
  },
  {
    "timestamp": 1729785660,
    "head": 12346,
    "headers": 98770,
    "ok": true,
    "reason": "ok"
  },
  ...
]
```

### `data/batch.json`

```json
{
  "n": 60,
  "good": 58,
  "threshold": 57,
  "bitmap_hash": "a1b2c3d4e5f6789...",
  "window": {
    "start": 1729785600,
    "end": 1729789200
  }
}
```

### `data/bitmap.hex`

```
0101010101010101010101010101010101010101010101010101010101010101...
```

(Each byte is 01 for ok, 00 for not ok)

## 🧪 Testing Tips

### For Quick Testing (shorter intervals):

Edit `config.toml`:

```toml
[sampling]
tick_secs = 10        # Sample every 10 seconds
window_secs = 60      # Generate batch every minute
max_staleness_secs = 30
```

This will:

- Sample every 10 seconds
- Generate batches every minute (6 samples per batch)
- Easier to test without waiting hours

### Simulating Failures

To see how the system handles node failures:

1. Stop sending metrics (staleness will trigger)
2. Send metrics with same head value (head advancement will fail)
3. Send metrics with same headers value (headers advancement will fail)

## 🎯 Next Steps (Not Yet Implemented)

According to the PRD, the following are future work:

- **Section 5.5**: ZK proof generation
- **Section 5.6**: Actual Celestia DA posting
- **Section 5.7**: Dashboard UI

For now, we're printing what **would** be posted to DA, which is perfect for validating the business logic.

## 🔍 Monitoring

Watch the logs for:

- `✓ Sample #...` - Each sampling tick
- `📦 Batch ready` - Batch generation summary
- `Updated DAS head` / `Updated DAS headers` - Metrics ingestion
- Any errors or warnings about staleness or advancement

All files are written to the `data/` directory for inspection and debugging.
