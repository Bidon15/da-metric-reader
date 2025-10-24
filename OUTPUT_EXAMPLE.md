# Terminal Output Example

## Startup

```
🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
📊 Sampler will tick every 30 seconds
📦 Batches will be generated every 3600 seconds
🔄 Sampler started (tick every 30s, window size: 120)
📦 Batch generator started (every 3600s)
```

## When Metrics Are Received

```
📥 Received metrics - DAS metrics updated successfully
```

## Every 30 Seconds (Sample Tick)

### Successful Sample

```
✅ Sample OK - Head: Some(12345) (+2), Headers: Some(98765) | Buffer: 45/120 samples
```

This shows:

- Head value increased by 2 blocks
- Current header count
- Buffer has 45 samples out of 120 max

### Failed Sample

```
❌ Sample FAILED - head not advanced (prev: Some(12345), curr: Some(12345)) | Head: Some(12345), Headers: Some(98770)
```

## Every Hour (Batch Generation)

```
================================================================================
📦 BATCH GENERATED - Would post to Celestia DA
================================================================================
🕐 Time Window:
   Start: 1729785600 (2025-10-24 12:00:00 UTC)
   End:   1729789200 (2025-10-24 13:00:00 UTC)

📊 Statistics:
   Total Samples:     120
   Successful (OK):   118
   Failed:            2
   Uptime:            98.33%
   Threshold:         114 (95%)
   Meets Threshold:   ✅ YES

🔐 Cryptographic Data:
   Bitmap Hash:       a1b2c3d4e5f6789abcdef123456789...
   Bitmap Length:     120 bytes

📄 Files Written:
   - data/batch.json
   - data/bitmap.hex
   - data/samples.json

💾 What would be posted to DA:
{
  "batch": {
    "n": 120,
    "good": 118,
    "threshold": 114,
    "bitmap_hash": "a1b2c3d4e5f6789abcdef123456789...",
    "window": {
      "start": 1729785600,
      "end": 1729789200
    }
  },
  "namespace": "0x2N1CE",
  "timestamp": 1729789200
}
================================================================================

✅ Batch complete: n=120, good=118, threshold=114, uptime=98.33% - Ready for DA posting
🎉 Uptime threshold MET - This batch would be accepted!
```

## Timeline Example (First 2 Minutes)

```
00:00 🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
00:00 📊 Sampler will tick every 30 seconds
00:00 📦 Batches will be generated every 3600 seconds

00:15 📥 Received metrics - DAS metrics updated successfully
00:30 ✅ Sample OK - Head: Some(100) (+2), Headers: Some(500) | Buffer: 1/120 samples

00:45 📥 Received metrics - DAS metrics updated successfully
01:00 ✅ Sample OK - Head: Some(102) (+2), Headers: Some(510) | Buffer: 2/120 samples

01:15 📥 Received metrics - DAS metrics updated successfully
01:30 ✅ Sample OK - Head: Some(104) (+2), Headers: Some(520) | Buffer: 3/120 samples

... continues every 30 seconds ...
```

## What Each Component Means

### 📥 Metrics Received

- Triggered when OTLP endpoint receives metrics from your DAS node
- Only shows if DAS-specific metrics (`das_sampled_chain_head`, `das_total_sampled_headers`) are present
- Indicates fresh data is being ingested

### ✅ Sample OK

- **Head: Some(X)** - Current block height being sampled
- **(+N)** - How many blocks advanced since last sample (should be ≥1)
- **Headers: Some(Y)** - Total headers sampled count
- **Buffer: X/Y samples** - How many samples in the rolling window

### ❌ Sample FAILED

Shows specific reason for failure:

- `stale (age > 120s)` - Metrics too old
- `head not advanced` - Block height didn't increase
- `headers not advanced` - Header count didn't increase

### 📦 Batch Generation

- Happens every `window_secs` (default 3600s = 1 hour)
- Shows detailed statistics and what would be posted to Celestia DA
- Clearly indicates if uptime threshold is met (✅) or not (❌)

## Celestia Block Alignment

With **30-second ticks**:

- Celestia blocks: ~12-15 seconds
- Our samples: every 30 seconds
- **Result**: We sample every ~2 Celestia blocks ✅

This ensures we catch any DA issues quickly while avoiding excessive sampling.

## Quick Health Check

**Healthy System:**

```
✅ Sample OK - Head: Some(12345) (+2), Headers: Some(98765) | Buffer: 45/120 samples
✅ Sample OK - Head: Some(12347) (+2), Headers: Some(98770) | Buffer: 46/120 samples
✅ Sample OK - Head: Some(12349) (+2), Headers: Some(98775) | Buffer: 47/120 samples
```

**Unhealthy System:**

```
❌ Sample FAILED - head not advanced (prev: Some(12345), curr: Some(12345)) | Head: Some(12345), Headers: Some(98765)
❌ Sample FAILED - stale (age > 120s) | Head: Some(12345), Headers: Some(98765)
❌ Sample FAILED - headers not advanced (prev: Some(98765), curr: Some(98765)) | Head: Some(12347), Headers: Some(98765)
```

## Files Generated

All files are written to the `data/` directory:

- `samples.json` - Every sample with timestamps and OK/FAILED status
- `batch.json` - Batch metadata (updated every hour)
- `bitmap.hex` - Hex-encoded bitmap of all samples (updated every hour)
