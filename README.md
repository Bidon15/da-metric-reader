# DA Reader - DAS Node Monitoring for 2Nice DFP

A Rust-based monitoring agent for Celestia Data Availability Sampling (DAS) nodes that collects health metrics, generates verifiable uptime attestations, and posts them to Celestia DA for the Delegation Foundation Program (DFP).

## 🎯 Purpose

Provide **cryptographically verifiable proof** that DA node operators are maintaining uptime commitments by:

- Sampling node health metrics every 30 seconds
- Posting individual samples to Celestia DA (Layer 1: detailed audit trail)
- Generating batched attestations every 10 minutes with ZK proofs (Layer 2: efficient verification)

This enables a **trust-minimized Service Level Market** where node performance can be objectively measured and verified.

## ✨ Features

✅ **OTLP Metrics Ingestion** - Receives OpenTelemetry metrics from DAS nodes  
✅ **Health Sampling** - Every 30s checks if chain head is advancing  
✅ **Ring Buffer** - Maintains sliding window of samples for batching  
✅ **Batch Generation** - Every 10min creates attestation with uptime percentage  
✅ **Cryptographic Hashing** - BLAKE3 hash of bitmap for integrity  
✅ **File Persistence** - Saves samples, batches, and bitmaps locally  
✅ **DA Posting Ready** - Prepared for posting to Celestia DA  
✅ **ZK Proof Ready** - Structure prepared for Groth16 proof generation

## 🏗️ Architecture

### Code Structure

```
src/
├── main.rs              - Entry point & initialization
├── config.rs            - Configuration from config.toml
├── types.rs             - Data models & shared types
├── utils.rs             - Helper functions
│
├── otlp/                - OpenTelemetry Protocol handling
│   ├── mod.rs           - Parser & normalizer
│   └── handlers.rs      - HTTP endpoint handler
│
├── metrics/             - Metrics collection & processing
│   ├── sampler.rs       - Every-30s health checks
│   └── batch.rs         - Every-10min batch generation
│
├── da/                  - Data Availability layer (TODO)
│   └── mod.rs           - Celestia DA posting logic
│
└── storage/             - Persistence layer
    └── mod.rs           - File I/O operations
```

### Data Flow

```
┌─────────────────────────────────────────────────────────┐
│                    DAS Node                             │
│  Exposes OTLP/HTTP metrics (head height, headers, etc.) │
└──────────────────┬──────────────────────────────────────┘
                   │ POST /v1/metrics (every 5-10s)
                   ▼
┌─────────────────────────────────────────────────────────┐
│              DA Reader (this service)                   │
│  Port: 4318                                             │
│                                                          │
│  [OTLP Handler] ──▶ Parse & normalize metrics           │
│         │                                                │
│         ▼                                                │
│  [Sampler - 30s tick]                                   │
│    • Check head advancement                             │
│    • Check headers advancing                            │
│    • Check data freshness                               │
│    • Generate bit (0/1)                                 │
│    • Store in ring buffer                               │
│    • Save to samples.json                               │
│    • POST to DA Layer 1 ◀── TODO                        │
│         │                                                │
│         ▼                                                │
│  [Batch Generator - 10min]                              │
│    • Collect ring buffer                                │
│    • Calculate uptime %                                 │
│    • Hash bitmap (BLAKE3)                               │
│    • Save batch.json + bitmap.hex                       │
│    • Generate ZK proof ◀── TODO                         │
│    • POST to DA Layer 2 ◀── TODO                        │
└─────────────────────────────────────────────────────────┘
                   │
                   ▼
        ┌────────────────────┐
        │   Celestia DA      │
        │                    │
        │  Layer 1: Samples  │
        │  Layer 2: Batches  │
        └────────────────────┘
```

## 🚀 Quick Start

### 1. Configure

Edit `config.toml`:

```toml
[sampling]
tick_secs = 30              # Sample every 30 seconds
max_staleness_secs = 120    # Max metric age
grace_period_secs = 45      # Grace period for head advancement

[da_posting]
enabled = false             # Enable when ready to post to DA
post_every_sample = true    # Post each sample (Layer 1)

[batching]
window_secs = 600           # Generate batch every 10 minutes

[metrics]
head_metric = "das_sampled_chain_head"
headers_metric = "das_total_sampled_headers"
min_increment = 1

[celestia]
node_url = "http://localhost:26658"
namespace = "0x2N1CE"
poster_mode = "mock"        # or "real"

[proofs]
enabled = false
threshold_percent = 0.95    # 95% uptime threshold
```

### 2. Run

```bash
# Build
cargo build --release

# Run
cargo run --release

# You'll see:
# 🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
# 📊 Sampler will tick every 30 seconds
# 📦 Batches will be generated every 600 seconds (10 minutes)
```

### 3. Point Your DAS Node

Configure your DAS node to export metrics via OTLP/HTTP to `http://localhost:4318/v1/metrics`.

## 📊 Two-Layer DA Posting Strategy

### Layer 1: Individual Samples (Every 30s)

**What gets posted:**

```json
{
  "type": "sample",
  "timestamp": 1729785600,
  "ok": true,
  "reason": "+2 blocks"
}
```

**Purpose:** Complete audit trail - anyone can replay the entire history

### Layer 2: Batch Attestations (Every 10min)

**What gets posted:**

```json
{
  "type": "batch_attestation",
  "window": { "start": 1729785600, "end": 1729786200 },
  "summary": {
    "n": 20,
    "good": 19,
    "threshold": 19,
    "uptime_percent": 95.0
  },
  "bitmap_hash": "a1b2c3d4e5f6...",
  "zk_proof": "0x..."
}
```

**Purpose:** Efficient verification with cryptographic guarantees

See [`docs/DA_POSTING_STRATEGY.md`](docs/DA_POSTING_STRATEGY.md) for detailed explanation.

## 📁 Output Files

The service generates these files in the `data/` directory:

- **`samples.json`** - All individual health samples
- **`bitmap.hex`** - Binary bitmap of uptime (01 = ok, 00 = not ok)
- **`batch.json`** - Batch metadata with uptime statistics

Example batch output:

```
================================================================================
📦 BATCH GENERATED FOR ZK PROOF
================================================================================
🕐 Time Window:
   Start: 1729785600 (2024-10-24 12:00:00 UTC)
   End:   1729786200 (2024-10-24 12:10:00 UTC)

📊 Statistics:
   Total Samples:     20
   Successful (OK):   19
   Failed:            1
   Uptime:            95.00%
   Threshold:         19 (95%)
   Meets Threshold:   ✅ YES

🔐 Cryptographic Data:
   Bitmap Hash:       d4a7f92b8c3e1d6f...
   Bitmap Length:     20 bytes
================================================================================
```

## 🔍 Sampling Logic

The sampler evaluates three conditions every 30 seconds:

1. **Staleness Check** - Is data fresh (< 120s old)?
2. **Head Advancement** - Has chain head increased?
3. **Headers Advancement** - Have sampled headers increased?

All three must pass for the sample to be marked as "OK" (bit = 1).

See [`docs/SAMPLING_LOGIC.md`](docs/SAMPLING_LOGIC.md) for detailed logic.

## 📚 Documentation

All detailed documentation is in the [`docs/`](docs/) directory:

- **[PRD.md](docs/PRD.md)** - Product Requirements Document
- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture
- **[DA_POSTING_STRATEGY.md](docs/DA_POSTING_STRATEGY.md)** - Two-layer DA posting approach
- **[SAMPLING_LOGIC.md](docs/SAMPLING_LOGIC.md)** - How health checks work
- **[METRICS_IMPLEMENTATION.md](docs/METRICS_IMPLEMENTATION.md)** - Implementation details
- **[DATA_FLOW.md](docs/DATA_FLOW.md)** - Data flow diagrams
- **[LOGGING.md](docs/LOGGING.md)** - Logging strategy
- **[OUTPUT_EXAMPLE.md](docs/OUTPUT_EXAMPLE.md)** - Sample outputs
- **[USAGE_EXAMPLES.md](docs/USAGE_EXAMPLES.md)** - Code examples

## 🛠️ Development

### Testing with shorter intervals

For faster testing, modify `config.toml`:

```toml
[sampling]
tick_secs = 10          # Sample every 10s instead of 30s

[batching]
window_secs = 60        # Batch every 1 min instead of 10 min
```

This gives you 6 samples per batch instead of waiting 10 minutes.

### Simulating failures

To test failure detection:

1. Stop sending metrics (staleness triggers)
2. Send metrics with same head value (advancement fails)
3. Send metrics with same headers value (headers check fails)

## 🔮 Roadmap

### Phase 1: Core Metrics ✅

- [x] OTLP ingestion
- [x] Health sampling
- [x] Batch generation
- [x] File persistence

### Phase 2: DA Posting (In Progress)

- [ ] Celestia DA client integration
- [ ] Post samples to DA (Layer 1)
- [ ] Post batches to DA (Layer 2)

### Phase 3: ZK Proofs

- [ ] Groth16 proof generation (arkworks)
- [ ] Prove: Σ bits ≥ threshold
- [ ] Include proof in batch attestation

### Phase 4: Dashboard

- [ ] Web UI (Axum + Askama)
- [ ] Show recent batches
- [ ] Display DA commitments
- [ ] Verify proof status

## 🔧 Technology Stack

- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Axum** - Web framework
- **OpenTelemetry** - Metrics protocol
- **BLAKE3** - Cryptographic hashing
- **Serde** - Serialization
- **Tracing** - Structured logging

Future:

- **arkworks** - ZK proof generation
- **celestia-types** - Celestia DA API

## 🤝 Contributing

This is part of the 2Nice Delegation Foundation Program (DFP) for measuring and verifying DA node operator performance.

## 📄 License

MIT
