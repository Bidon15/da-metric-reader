# Architecture: Separated DA Posting and ZK Batching

## Key Insight

We've separated two distinct concerns that have different requirements:

| Concern         | Frequency          | Purpose                              | Cost               |
| --------------- | ------------------ | ------------------------------------ | ------------------ |
| **DA Posting**  | Every sample (30s) | Create detailed, replayable history  | Low (just storage) |
| **ZK Batching** | Every 5-10 min     | Prove uptime over meaningful windows | High (computation) |

## Why This Design?

### DA Posting: Frequent & Detailed

```
Every 30 seconds → Post sample to Celestia DA

Benefits:
✅ Detailed history - every sample recorded
✅ Replayable - anyone can verify each check
✅ Real-time visibility - see issues immediately
✅ Low cost - just append data to DA layer
```

### ZK Batching: Infrequent & Aggregated

```
Every 5-10 minutes → Generate ZK proof of batch

Benefits:
✅ Amortize proof cost over many samples
✅ Prove uptime over meaningful time window
✅ Smaller proof size than individual proofs
✅ Suitable for on-chain verification
```

## Data Flow

```
┌─────────────────┐
│   DAS Node      │
│  (OTLP Export)  │
└────────┬────────┘
         │ Every ~15s
         ▼
┌────────────────────────────────────────────────────────────┐
│                    da-reader                               │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  SAMPLING (Every 30s)                               │  │
│  │  ✓ Check head/headers advancing                     │  │
│  │  ✓ Generate sample bit (0 or 1)                     │  │
│  │  ✓ Save to samples.json                             │  │
│  └──────────────────┬──────────────────────────────────┘  │
│                     │                                      │
│                     ├─────────────────┬───────────────────┤
│                     │                 │                   │
│                     ▼                 ▼                   │
│  ┌──────────────────────────┐  ┌─────────────────────┐   │
│  │  DA POSTING (Every 30s)  │  │ BATCHING (5-10 min) │   │
│  │  Post each sample to DA  │  │ Aggregate samples   │   │
│  │  for detailed history    │  │ for ZK proof        │   │
│  └──────────┬───────────────┘  └─────────┬───────────┘   │
│             │                             │               │
└─────────────┼─────────────────────────────┼───────────────┘
              │                             │
              ▼                             ▼
    ┌──────────────────┐         ┌──────────────────┐
    │  Celestia DA     │         │  ZK Proof Gen    │
    │  (Every sample)  │         │  (Every batch)   │
    │  ─ Sample bits   │         │  ─ Batch proof   │
    │  ─ Timestamps    │         │  ─ Bitmap hash   │
    │  ─ Head/Headers  │         │  ─ Threshold     │
    └──────────────────┘         └──────────────────┘
```

## Configuration

### In `config.toml`:

```toml
[sampling]
tick_secs = 30          # Sample health every 30 seconds
max_staleness_secs = 120
grace_period_secs = 45

[da_posting]
enabled = false         # Enable when ready
post_every_sample = true # Post each sample (recommended for detailed history)

[batching]
window_secs = 600       # 10 minutes = 20 samples per batch
                        # Balance between proof cost and time resolution
```

### Tuning Guide

#### Sampling Interval (`tick_secs`)

```
15s: Very frequent, detailed (80 samples/batch @ 10min)
30s: Recommended - aligns with ~5 Celestia blocks ✅
60s: Less frequent (10 samples/batch @ 10min)
```

#### Batching Window (`window_secs`)

```
300s  (5 min):  10 samples - Quick proofs, frequent verification
600s  (10 min): 20 samples - Good balance ✅
900s  (15 min): 30 samples - Longer windows, fewer proofs
1800s (30 min): 60 samples - Even longer windows
```

**Trade-offs:**

- **Shorter windows**: More proofs, higher cost, faster feedback
- **Longer windows**: Fewer proofs, lower cost, slower feedback

## Example Timeline

```
Time    | Sampling          | DA Posting                | Batching
--------|-------------------|---------------------------|------------------
00:00   | Sample #1 ✅      | Post sample #1 to DA      |
00:30   | Sample #2 ✅      | Post sample #2 to DA      |
01:00   | Sample #3 ✅      | Post sample #3 to DA      |
...     | ...               | ...                       |
09:30   | Sample #19 ✅     | Post sample #19 to DA     |
10:00   | Sample #20 ✅     | Post sample #20 to DA     | Generate batch #1
        |                   |                           | (20 samples)
        |                   |                           | Create ZK proof
10:30   | Sample #21 ✅     | Post sample #21 to DA     |
...     | ...               | ...                       |
20:00   | Sample #40 ✅     | Post sample #40 to DA     | Generate batch #2
        |                   |                           | (20 samples)
```

## What Gets Posted Where

### Celestia DA (Every Sample)

```json
{
  "type": "sample",
  "timestamp": 1729785600,
  "ok": true,
  "head": 8549695,
  "headers": 8549717,
  "reason": "+2 blocks"
}
```

**Size**: ~100-200 bytes per sample  
**Frequency**: Every 30 seconds  
**Purpose**: Detailed, replayable history

### ZK Proof (Every Batch)

```json
{
  "type": "batch",
  "window": {
    "start": 1729785600,
    "end": 1729786200
  },
  "n": 20,
  "good": 19,
  "threshold": 19,
  "bitmap_hash": "a1b2c3d4...",
  "proof": "0x..."
}
```

**Size**: ~1-2 KB per batch  
**Frequency**: Every 5-10 minutes  
**Purpose**: Provable uptime attestation

## Benefits of This Architecture

### 1. **Detailed History**

Every check is recorded on DA → anyone can replay and verify

### 2. **Cost-Effective Proofs**

Aggregate 20 samples → generate 1 proof (amortized cost)

### 3. **Real-Time Visibility**

Samples posted every 30s → see issues immediately

### 4. **Flexible Verification**

- Quick check: Look at DA samples
- Cryptographic proof: Verify ZK proof
- Full audit: Replay entire history

### 5. **Aligned with Celestia**

30s samples = ~5 Celestia blocks  
Detailed enough to catch block-level issues

## Implementation Status

| Component           | Status             | Notes                                |
| ------------------- | ------------------ | ------------------------------------ |
| Sampling            | ✅ Implemented     | Working with 30s interval            |
| Ring buffer         | ✅ Implemented     | Stores samples for batching          |
| Batch generation    | ✅ Implemented     | Creates batches every 10 min         |
| File output         | ✅ Implemented     | samples.json, batch.json, bitmap.hex |
| DA posting          | ❌ Not implemented | Placeholder in code                  |
| ZK proof generation | ❌ Not implemented | Planned (arkworks)                   |
| Batch signing       | ❌ Not implemented | Planned (ed25519)                    |

## Next Steps

### Phase 1: DA Posting (Current Priority)

```rust
// After each sample:
if config.da_posting.enabled {
    post_sample_to_celestia_da(&sample).await;
}
```

**Implementation:**

- Connect to Celestia light node API
- Format sample as blob
- Post to namespace `0x2N1CE`
- Handle errors/retries

### Phase 2: ZK Proof Generation

```rust
// After each batch:
if config.proofs.enabled && meets_threshold {
    let proof = generate_zk_proof(&batch).await;
    verify_proof(&proof, &public_inputs);
}
```

**Implementation:**

- Use arkworks (Groth16 + BN254)
- Prove: Σ bits ≥ threshold
- Verify locally first
- Post proof + public inputs to DA

### Phase 3: End-to-End Flow

```
Sample → DA → Batch → ZK Proof → DA → Verifiable Receipt
```

## Cost Analysis

Assuming Celestia DA costs and 30s sampling:

### Per Hour

- Samples: 120 samples × ~150 bytes = 18 KB
- Batches: 6 batches × ~2 KB = 12 KB
- **Total**: ~30 KB/hour

### Per Day

- Samples: 2,880 samples × ~150 bytes = 432 KB
- Batches: 144 batches × ~2 KB = 288 KB
- **Total**: ~720 KB/day

Very affordable for Celestia DA! 💰

## Summary

This architecture **separates concerns**:

- DA posting = detailed history (frequent, cheap)
- ZK batching = proof generation (infrequent, expensive)

This gives you both:

1. **Transparency** - every check is recorded
2. **Efficiency** - proofs are generated over batches
3. **Flexibility** - tune frequencies independently
