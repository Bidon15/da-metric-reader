# DA Posting Strategy - Two-Layer Approach

## Overview

We post **TWO types of data** to Celestia DA, each serving a different purpose:

| Layer                | Frequency    | Content                  | Purpose                      | Size       |
| -------------------- | ------------ | ------------------------ | ---------------------------- | ---------- |
| **Layer 1: Samples** | Every 30s    | Individual health checks | Detailed, replayable history | ~150 bytes |
| **Layer 2: Batches** | Every 10 min | Aggregated proof         | Quick verification           | ~2 KB      |

## Why Post Both?

### Layer 1: Individual Samples (Every 30s)

**What gets posted:**

```json
{
  "type": "sample",
  "timestamp": 1729785600,
  "ok": true,
  "head": 8549695,
  "headers": 8549717,
  "reason": "+2 blocks",
  "service": "das-node-1"
}
```

**Why this matters:**

- ✅ **Full audit trail** - Anyone can replay the entire history
- ✅ **Granular visibility** - See exactly when issues occurred
- ✅ **Tamper-proof** - Can't cherry-pick good samples
- ✅ **Real-time** - Issues are recorded within 30 seconds

**Use cases:**

- Dispute resolution: "Show me what happened at 12:35 PM"
- Root cause analysis: "The node was down for exactly 2 samples (1 minute)"
- Trust verification: "Prove you didn't manipulate the data"

---

### Layer 2: Batches + ZK Proofs (Every 10 min)

**What gets posted:**

```json
{
  "type": "batch_attestation",
  "window": {
    "start": 1729785600,
    "end": 1729786200
  },
  "summary": {
    "n": 20,
    "good": 19,
    "threshold": 19,
    "uptime_percent": 95.0
  },
  "bitmap_hash": "a1b2c3d4e5f6...",
  "zk_proof": "0x...",
  "public_inputs": {
    "n": 20,
    "threshold": 19,
    "bitmap_hash": "a1b2c3d4e5f6..."
  }
}
```

**Why this matters:**

- ✅ **Efficient verification** - Check proof without replaying 20 samples
- ✅ **Cryptographic guarantee** - ZK proof ensures correctness
- ✅ **Smaller data** - 1 batch vs 20 individual samples
- ✅ **SLA compliance** - Quickly verify uptime meets threshold

**Use cases:**

- Quick verification: "Did this operator meet 95% uptime? (check proof)"
- On-chain settlement: "Post proof to smart contract for payment"
- Reporting: "Show last 10 batches to see trend"

## Data Flow to Celestia DA

```
┌─────────────────────────────────────────────────────────┐
│                  DAS Node Monitoring                    │
└─────────────────────┬───────────────────────────────────┘
                      │
                      ▼
          ┌───────────────────────┐
          │  Sample Every 30s     │
          │  ✓ Check health       │
          │  ✓ Generate bit (0/1) │
          └───────────┬───────────┘
                      │
                      ├──────────────┬──────────────────┐
                      │              │                  │
                      ▼              ▼                  ▼
          ┌──────────────────┐  ┌──────────┐  ┌──────────────┐
          │ Ring Buffer (20) │  │ Save to  │  │  POST TO DA  │
          │ For batching     │  │ Local    │  │  (Sample)    │
          └──────────────────┘  │ Files    │  │  Layer 1     │
                      │         └──────────┘  └──────────────┘
                      │                               │
                      │ Every 10 min                  │ Every 30s
                      ▼                               ▼
          ┌───────────────────────┐      ┌──────────────────────┐
          │  Generate Batch       │      │  Celestia DA Blob    │
          │  ✓ Calculate uptime   │      │  Type: sample        │
          │  ✓ Hash bitmap        │      │  Size: ~150 bytes    │
          │  ✓ Generate ZK proof  │      │  Queryable: yes      │
          └───────────┬───────────┘      └──────────────────────┘
                      │
                      ▼
          ┌───────────────────────┐
          │  POST TO DA           │
          │  (Batch + ZK Proof)   │
          │  Layer 2              │
          └───────────┬───────────┘
                      │
                      ▼
          ┌──────────────────────┐
          │  Celestia DA Blob    │
          │  Type: batch         │
          │  Size: ~2 KB         │
          │  Queryable: yes      │
          └──────────────────────┘
```

## Verification Scenarios

### Scenario 1: Quick Check (Use Layer 2)

**Question:** Did the operator meet 95% uptime in the last hour?

**Process:**

1. Query DA for last 6 batches (6 × 10 min = 1 hour)
2. Verify each ZK proof (fast, cryptographic)
3. Check: `good >= threshold` for each batch

**Time:** Seconds (verify 6 proofs)  
**Data:** 6 × 2 KB = 12 KB

---

### Scenario 2: Detailed Audit (Use Layer 1)

**Question:** Show me exactly when the node failed on Tuesday.

**Process:**

1. Query DA for all samples on Tuesday (2,880 samples)
2. Filter by `ok == false`
3. Show timestamps, reasons, and head values

**Time:** Minutes (retrieve and analyze 2,880 samples)  
**Data:** 2,880 × 150 bytes = 432 KB

---

### Scenario 3: Dispute Resolution (Use Both Layers)

**Claim:** "I had 98% uptime!"  
**Counter:** "Your ZK proof shows only 92%"

**Process:**

1. Check Layer 2: Batch shows 92% uptime with valid proof
2. Check Layer 1: Retrieve all 20 samples from that window
3. Verify: Count shows 18/20 good = 90% (not 92% or 98%)
4. Resolution: Data is on-chain, immutable, provable

**Outcome:** Truth prevails ✅

## Cost Analysis

### Per Hour (Celestia DA)

**Layer 1 (Samples):**

- 120 samples × 150 bytes = 18 KB
- Cost: ~$0.XX (varies by DA fee)

**Layer 2 (Batches):**

- 6 batches × 2 KB = 12 KB
- Cost: ~$0.XX (varies by DA fee)

**Total:** ~30 KB/hour

### Per Day

**Layer 1:** 432 KB  
**Layer 2:** 288 KB  
**Total:** ~720 KB/day

**Very affordable!** 💰

## Implementation Status

| Component           | Status     | Notes                    |
| ------------------- | ---------- | ------------------------ |
| Sample generation   | ✅ Working | Every 30s                |
| Batch generation    | ✅ Working | Every 10 min             |
| Local file output   | ✅ Working | samples.json, batch.json |
| **Layer 1 posting** | ❌ TODO    | Post each sample to DA   |
| **Layer 2 posting** | ❌ TODO    | Post batch + proof to DA |
| ZK proof generation | ❌ TODO    | Groth16/BN254            |

## Next Steps

### Phase 1: Implement DA Posting

```rust
// Layer 1: Post each sample
async fn post_sample_to_da(sample: &SampleBit, state: &AppState) -> Result<()> {
    let blob = create_blob(
        &state.config.celestia.namespace,
        "sample",
        sample
    );

    let commitment = celestia_client
        .submit_blob(blob)
        .await?;

    info!("📡 Posted sample to DA: {}", commitment);
    Ok(())
}

// Layer 2: Post batch + proof
async fn post_batch_to_da(
    batch: &Batch,
    proof: &ZkProof,
    state: &AppState
) -> Result<()> {
    let attestation = BatchAttestation {
        batch: batch.clone(),
        proof: proof.clone(),
        timestamp: now(),
    };

    let blob = create_blob(
        &state.config.celestia.namespace,
        "batch_attestation",
        &attestation
    );

    let commitment = celestia_client
        .submit_blob(blob)
        .await?;

    info!("📡 Posted batch attestation to DA: {}", commitment);
    Ok(())
}
```

### Phase 2: ZK Proof Generation

Generate Groth16 proof that proves: `Σ bits >= threshold`

### Phase 3: Query Interface

Allow verifiers to query DA:

```bash
# Get all samples in a time range
query_samples --from 1729785600 --to 1729789200

# Get batch attestations
query_batches --from 1729785600 --to 1729789200

# Verify a specific batch
verify_batch --commitment abc123...
```

## Summary

**Two-layer approach gives you the best of both worlds:**

1. **Layer 1 (Samples)** = Complete transparency, full audit trail
2. **Layer 2 (Batches)** = Efficient verification, cryptographic proofs

**Key Insight:** Anyone can verify a batch proof quickly (Layer 2), but if there's any dispute, they can always go back to the raw samples (Layer 1) which are immutably stored on DA.

This creates a **trust-minimized system** where:

- ✅ You can't fake uptime (samples are on DA)
- ✅ You can't cherry-pick data (all samples are posted)
- ✅ Verification is efficient (ZK proofs)
- ✅ Disputes are resolvable (raw data available)

Perfect for a Service Level Market! 🎯
