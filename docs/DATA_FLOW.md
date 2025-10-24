# Data Flow - Where Things Go

## Overview

```
┌─────────────────┐
│   DAS Node      │  Your Celestia DAS node
│  (OTLP Export)  │  Sends metrics every ~15s
└────────┬────────┘
         │ OTLP/HTTP (protobuf/gzip)
         │
         ▼
┌──────────────────────────────────────────────────────────────────┐
│               da-reader (This Application)                       │
│                                                                  │
│  1. Receive OTLP    →  2. Sample Every 30s                      │
│     metrics             ├─ Check head advancing                 │
│     from DAS node       ├─ Check headers advancing              │
│                         ├─ Store in ring buffer (20 samples)    │
│                         └─ Post to DA (when enabled)            │
│                                                                  │
│  3. Generate Batch  →  4. ZK Proof (future)                     │
│     every 10 min        ├─ Print batch summary                  │
│     (20 samples)        ├─ Save to data/ files                  │
│                         └─ Generate ZK proof                    │
└──────────────────────────────────────────────────────────────────┘
         │                            │
         │ (Future: DA posting)       │ (Future: ZK proofs)
         ▼                            ▼
┌─────────────────┐          ┌─────────────────┐
│  Celestia DA    │          │  ZK Proof Gen   │
│  (Every 30s)    │          │  (Every 10 min) │
│  NOT YET        │          │  NOT YET        │
└─────────────────┘          └─────────────────┘
```

## Step-by-Step Log Flow

### Step 1: Receiving Metrics (Every ~15 seconds)

```
📥 Received OTLP metrics from DAS node - Stored internally
```

**What's happening:**

- Your DAS node sends OTLP metrics to `http://localhost:4318/v1/metrics`
- We decode the protobuf/gzip payload
- We extract `das_sampled_chain_head` and `das_total_sampled_headers`
- We store them **internally in memory** (NOT posted anywhere)

**Files affected:** None (just in-memory storage)

---

### Step 2: Sampling (Every 30 seconds)

```
✅ Sample OK - Head: Some(8549695) (+2 blocks), Headers: Some(8549717) | Buffer: 15/20 samples
```

**What's happening:**

- Timer ticks every 30 seconds
- We check if head/headers advanced
- We create a sample bit (0 or 1)
- We add it to the ring buffer
- We save to `data/samples.json`

**Files affected:**

- `data/samples.json` (updated every sample)

**Where posted:**

- If DA posting enabled: Posted to Celestia DA (every 30s)
- Otherwise: Just local storage

---

### Step 3: Batch Generation (Every 10 minutes)

```
================================================================================
📦 BATCH GENERATED FOR ZK PROOF
   This batch is for generating ZK proofs of uptime
   (Individual samples are posted to DA separately)
================================================================================
🕐 Time Window:
   Start: 1729785600 (2025-10-24 12:00:00 UTC)
   End:   1729786200 (2025-10-24 12:10:00 UTC)

📊 Statistics:
   Total Samples:     20
   Successful (OK):   19
   Failed:            1
   Uptime:            95.00%
   Threshold:         19 (95%)
   Meets Threshold:   ✅ YES

🔐 Cryptographic Data:
   Bitmap Hash:       a1b2c3d4e5f6...
   Bitmap Length:     20 bytes

📄 Files Written:
   - data/batch.json
   - data/bitmap.hex
   - data/samples.json

💾 Batch metadata for ZK proof:
{
  "batch": {
    "n": 20,
    "good": 19,
    "threshold": 19,
    "bitmap_hash": "a1b2c3d4e5f6...",
    "window": {
      "start": 1729785600,
      "end": 1729786200
    }
  },
  "namespace": "0x2N1CE",
  "timestamp": 1729786200
}
================================================================================

✅ Batch generated: n=20, good=19, threshold=19, uptime=95.00%
🎉 Uptime threshold MET (95%) - Batch ready for ZK proof generation
💾 Batch files saved to data/ directory (batch.json, bitmap.hex)
🔐 TODO: Generate ZK proof from this batch
📡 Individual samples already posted to DA (or will be when DA posting enabled)
```

**What's happening:**

- Every 10 minutes, we look at all 20 samples in the ring buffer
- We calculate uptime percentage
- We create a BLAKE3 hash of the bitmap
- We generate a JSON payload showing what WOULD be posted
- We save files locally

**Files affected:**

- `data/batch.json` (batch metadata)
- `data/bitmap.hex` (bitmap of all 20 samples)
- `data/samples.json` (already updated)

**Where posted:**

- Batch metadata: Saved locally (for ZK proof generation)
- Individual samples: Posted to DA every 30s (when enabled)

---

## What's Internal vs External

### Internal (Happening Now)

| Action          | Log                                      | Where                                            |
| --------------- | ---------------------------------------- | ------------------------------------------------ |
| Receive metrics | `📥 Received OTLP metrics from DAS node` | In-memory storage                                |
| Sample health   | `✅ Sample OK`                           | Ring buffer + `data/samples.json`                |
| Generate batch  | `📦 BATCH GENERATED`                     | Terminal + `data/batch.json` + `data/bitmap.hex` |

### External (NOT Implemented Yet)

| Action              | Status             | When Implemented                                                        |
| ------------------- | ------------------ | ----------------------------------------------------------------------- |
| Post to Celestia DA | ❌ Not implemented | Would show: `🚀 Posted to Celestia DA: height=12345, commitment=abc...` |
| Generate ZK proof   | ❌ Not implemented | Would show: `🔐 ZK proof generated and verified`                        |
| Sign batch          | ❌ Not implemented | Would show: `✍️  Batch signed with key: xyz...`                         |

---

## File System

All files are written to `data/` directory:

```
data/
├── samples.json       ← Updated every 30s (all samples)
├── batch.json        ← Updated every 10 minutes (batch metadata for ZK proofs)
└── bitmap.hex        ← Updated every 10 minutes (bitmap of 20 samples)
```

**These are LOCAL files** - not posted to DA yet.

---

## Key Logs to Understand

### ✅ Internal Operations (Working Now)

```
📥 Received OTLP metrics from DAS node - Stored internally
```

= We got metrics FROM your DAS node and stored them in memory

```
✅ Sample OK - Head: Some(X) (+N blocks)
```

= We checked health and created a sample bit

```
📦 BATCH GENERATED (NOT YET POSTED TO DA)
```

= We created a batch locally, files saved, NOT posted anywhere

```
💾 Batch files saved to data/ directory
```

= Files written to local filesystem

```
🚀 TODO: Implement actual Celestia DA posting
```

= Reminder that we're NOT posting to DA yet

### ❌ Future Operations (Not Implemented)

When DA posting is implemented, you'd see:

```
🚀 Posting batch to Celestia DA...
✅ Posted to DA: height=12345, namespace=0x2N1CE, commitment=abc...
```

When signing is implemented:

```
✍️  Signing batch with key: abc123...
✅ Batch signed successfully
```

When ZK proofs are implemented:

```
🔐 Generating ZK proof (this may take a minute)...
✅ Proof generated and verified
```

---

## Summary

**Right now, the system:**

1. ✅ Receives metrics from your DAS node (OTLP/HTTP)
2. ✅ Samples health every 30 seconds
3. ✅ Generates batches every 10 minutes (20 samples per batch)
4. ✅ Saves everything to local files
5. ✅ Shows batch metadata for ZK proof generation
6. ❌ Does NOT actually post to Celestia DA yet (placeholder in code)
7. ❌ Does NOT generate ZK proofs yet

**Architecture:**

- **DA Posting**: Every sample (30s) → Detailed history
- **ZK Batching**: Every batch (10 min) → Prove uptime

**Next steps (when you're ready):**

- Implement actual Celestia DA posting via light node API (post each sample)
- Add ZK proof generation with arkworks (Groth16/BN254)
- Add batch signing with ed25519
