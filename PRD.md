# 🧩 Product Requirements Document (PRD)

**Project Name:** 2Nice DFP Metrics MVP  
**Author:** Nguyen  
**Date:** October 2025  
**Version:** v0.1  
**Status:** Draft for review

---

## 1. 🎯 Purpose

The goal is to **prove that validator/DA operator performance can be measured, signed, and verifiably committed to Celestia DA** in a decentralized, cryptographically verifiable way.

This MVP will serve as a proof-of-concept for the future **Service Level Market (SLM)** concept — where “work performed” by infrastructure participants can be measured and settled objectively.

---

## 2. 🧠 Problem Statement

- The **Delegation Foundation Program (DFP)** currently tracks validator activity using dashboards and telemetry, but **lacks verifiable, tamper-proof service receipts**.
- Node performance (e.g., uptime, block progress) is reported through off-chain monitoring tools with no cryptographic auditability.
- To enable measurable, incentive-driven operations, we must anchor service metrics directly into **Celestia DA blobs**.

---

## 3. ✅ Goals & Non-Goals

| Type         | Description                                                                                                      |
| ------------ | ---------------------------------------------------------------------------------------------------------------- |
| **Goal**     | Build a lightweight agent that receives metrics from DA Nodes and converts them into verifiable, signed batches. |
| **Goal**     | Post those batches to Celestia DA as immutable receipts.                                                         |
| **Goal**     | Generate optional zk proofs that the uptime ≥ a threshold (e.g. 95%).                                            |
| **Goal**     | Build a simple dashboard to visualize batches and their DA blob commitments.                                     |
| **Non-Goal** | Marketplace / payment logic (to be done in SLM phase).                                                           |
| **Non-Goal** | Multi-tenant configuration, key management, or full zk integration into chain contracts.                         |

---

## 4. 🏗️ High-Level Architecture

        ┌────────────────────────────────────────────────┐
        │                 DFP DA Node                    │
        │  ─ exposes OTLP/HTTP metrics (head height,     │
        │    peer count, etc.)                           │
        └────────────────────────────────────────────────┘
                            │  push (OTLP HTTP)
                            ▼
        ┌────────────────────────────────────────────────┐
        │              2Nice Metrics Agent (Rust)         │
        │  - HTTP /v1/metrics endpoint                    │
        │  - Decodes OTLP protobuf payloads               │
        │  - Samples at fixed interval → ok ∈ {0,1}       │
        │  - Maintains rolling window bitmap              │
        │  - Batches window → batch.json                  │
        │  - Signs batch → batch.signed.json              │
        │  - Optionally proves zk (Σ bits ≥ threshold)    │
        │  - Posts to Celestia DA via celestia-types      │
        └────────────────────────────────────────────────┘
                            │
                            ▼
        ┌────────────────────────────────────────────────┐
        │          Celestia DA (Light Node API)           │
        │   Stores blob with signed batch + optional zk   │
        └────────────────────────────────────────────────┘
                            │
                            ▼
        ┌────────────────────────────────────────────────┐
        │        Local Dashboard (Axum + Askama)          │
        │   Visualize uptime %, blob hash, proof status   │
        └────────────────────────────────────────────────┘

---

## 5. 🧩 Functional Requirements

### 5.1 Metric Ingestion (Light Node / DAS)

- Input: OTLP/HTTP (protobuf) POST /v1/metrics.
- Primary metrics:
- das_sampled_chain_head (Gauge) — latest sampled DA block height
- das_total_sampled_headers (Gauge) — cumulative headers sampled
- Semantics: both are monotonic when the node is healthy and sampling.
- Storage: in-memory latest values with timestamps.

### 5.2 Sampler (DAS-based liveness)

- Tick: every tick_secs (default 60s) produce a single bit.
- Predicate: ok = head_advanced_by ≥ 1 AND points_fresh ≤ max_staleness_secs
- Optional cross-check: require total_sampled_headers to advance as well.
- Output: append ok to ring buffer; window size defaults to 288.

### 5.3 Batch Generator

- Every `window_secs` (default 1h):
  - Computes `good = Σ bits`, `n = len(bits)`
  - Computes `threshold = ceil(0.95 * n)`
  - Hashes bitmap → `blake3(bitmap || salt)`
  - Writes `batch.json` + `bitmap.hex`
- Example:
  ```json
  {
    "n": 60,
    "good": 58,
    "threshold": 57,
    "bitmap_hash": "a1b2c3...",
    "window": { "start": 1732544000, "end": 1732547600 }
  }
  ```

### 5.5 Prover (optional)

- Reads `bitmap.hex` and `public.json` (which contains `n`, `threshold`, and `bitmap_hash`).
- Generates a zk proof that the number of `true` bits (Σ bits) in the bitmap is **greater than or equal to** the threshold.
- Outputs:
  - `proof.bin` — serialized Groth16 proof
  - `public.json` — the public inputs for verification
- Uses **arkworks** libraries:
  - `ark-bn254`, `ark-groth16`, `ark-relations`, `ark-serialize`
- Supports two proving modes:
  1. **Local mode** — generated and verified locally for demo.
  2. **Async mode (stretch)** — proof generation runs asynchronously and posts later.

---

### 5.6 Poster

- Posts the signed batch (and optionally the proof) to **Celestia DA** via the `celestia-types` crate.
- Supports two posting modes:
  - `real`: connects to a live or local Celestia light node endpoint (`http://localhost:26658`)
  - `mock`: writes the blob content to a local file for offline testing
- Each blob includes at least:

  ```json
  {
    "batch_signed": {...},
    "public": {...},
    "proof": "optional"
  }
  ```

- Returns and logs:
  - Blob commitment
  - Height
  - Namespace ID
  - Timestamp

### 5.7 Dashboard

- Built with **Axum** (web framework) and **Askama** (templating engine).
- Runs locally on `http://localhost:8080`.
- Responsibilities:
  - Serve an HTML dashboard at `/` displaying recent batch summaries:
    - Time window
    - Calculated uptime %
    - Blob commitment hash
    - ZK proof verification status ✅ / ❌
  - Expose a lightweight REST endpoint `/api/batches` returning the same data as JSON.
- The dashboard auto-refreshes every 30 seconds (stretch goal) to show new posted blobs in near real time.

---

## 6. 🧰 Technical Stack

| Layer           | Technology                   |
| --------------- | ---------------------------- |
| Language        | Rust (2025 stable)           |
| Async runtime   | Tokio                        |
| Web framework   | Axum                         |
| Templating      | Askama                       |
| Protobuf types  | opentelemetry-proto          |
| Hashing         | blake3                       |
| Signatures      | ed25519-dalek                |
| ZK proofs       | arkworks (Groth16 / BN254)   |
| Celestia DA API | celestia-types crate         |
| Logging         | tracing / tracing-subscriber |

---

## 7. ⚙️ Config Options

Example `config.toml`:

```toml
[sampling]
tick_secs = 60
window_secs = 3600
max_staleness_secs = 120

[metrics]
head_metric = "das_sampled_chain_head"
headers_metric = "das_total_sampled_headers"
min_increment = 1

[celestia]
node_url     = "http://localhost:26658"
namespace    = "0x2N1CE"
poster_mode  = "real"   # or "mock"

[proofs]
enabled            = true
threshold_percent  = 0.95
```

## 8. 📦 Outputs

| File                     | Purpose                                            |
| ------------------------ | -------------------------------------------------- |
| `data/samples.json`      | Raw metric samples collected per tick              |
| `data/bitmap.hex`        | Serialized bitmap of uptime bits                   |
| `data/batch.json`        | Unsigned batch metadata                            |
| `data/batch.signed.json` | Signed attestation (operator → foundation)         |
| `data/public.json`       | Public zk inputs (`n`, `threshold`, `bitmap_hash`) |
| `data/proof.bin`         | zk proof (Groth16)                                 |
| `data/blob.txt`          | Celestia DA blob commitment + height               |
| `data/history.json`      | Cached summary for dashboard/API                   |

---

## 9. 🧮 KPIs / Success Metrics

| KPI                          | Target                                         |
| ---------------------------- | ---------------------------------------------- |
| End-to-end reporting latency | < 3 minutes                                    |
| Blob submission success rate | ≥ 95 %                                         |
| Proof verification accuracy  | 100 % (valid inputs)                           |
| Demo completeness            | Metric → Batch → Blob → Dashboard in ≤ 2 weeks |

---

## 10. 🚦 Milestones

| Week                 | Deliverable                                              |
| -------------------- | -------------------------------------------------------- |
| **Week 1**           | OTLP HTTP receiver + sampler printing metrics to console |
| **Week 1.5**         | Ring buffer + batch writer + signature logic             |
| **Week 2**           | Blob posting to Celestia + dashboard UI                  |
| **Week 2 (stretch)** | zk proof generation + verification integration           |
| **Week 2 (final)**   | Full demo showing blob hash and proof on dashboard       |
