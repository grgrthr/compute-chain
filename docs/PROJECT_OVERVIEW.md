# Compute Chain — Project Overview

**Version:** 0.1.0-demo
**Date:** 2026-08-11
**Status:** Working Prototype

---

# Compute Chain — Project Overview

**Version:** 0.1.0-demo
**Status:** Working Prototype

A working prototype demonstrating verifiable compute with real browser workers.

---

## 1. The Problem

Traditional compute networks waste effort on fixed-purpose hashing.

Compute Chain explores a different approach:
- Useful workloads instead of arbitrary puzzles
- Verifiable results — every output can be independently checked
- Real devices — phones and laptops as workers, not specialized hardware

"What if computation itself could be verified and rewarded?"

---

## 2. The Vision

A network where compute work is verifiable.

BROWSER WORKERS -> REAL COMPUTE -> VERIFICATION -> PROOF -> REWARD

| Aspect | Vision | Current Demo |
|--------|--------|-------------|
| Workers | Many devices | 1 phone browser |
| Consensus | Decentralized PBFT | Single-node |
| Workloads | WASM, Docker, ML | Thumbnail, Hash, CSV |
| Network | Production P2P | Local network |

The demo proves the core pipeline. Production scale is the next phase.

---

## 3. Current Implementation

A complete end-to-end pipeline — working now:

Job Submission -> Browser Worker -> Real Execution -> Result Verified -> Merkle Commitment -> STARK Commitment Proof -> Block Accepted -> Reward Distributed -> Live Dashboard

All components built and tested.

---

## 4. Validation Evidence

Verified on two real devices.

| Metric | Result |
|--------|--------|
| Devices | Laptop (server) + Phone (worker) |
| Jobs completed | 6 consecutive jobs |
| Rewards distributed | 60 tokens total |
| Worker status | Returned to Idle after each job |
| Dashboard | Updated live, no refresh needed |
| Tests passing | 278 unit + 1 integration = 279 total (0 failures) |

Evidence: docs/VALIDATION_EVIDENCE.md
Video: https://youtube.com/shorts/_CMj4qW8IQM
Repository: https://github.com/grgrthr/compute-chain

---

## 5. Proof Model

The STARK proof is a COMMITMENT PROOF.

It proves that verified result data is bound to a Merkle root:
input hash, output hash, worker ID, task type, job ID.

It does NOT prove JavaScript execution inside the browser.

This commitment proof demonstrates cryptographic binding of verified results.

---

## 6. Current Limitations

| Capability | Current State |
|------------|---------------|
| Decentralized consensus | Not demonstrated; single-node |
| Production authentication | Not implemented |
| Persistent key management | Not implemented |
| P2P production hardening | Not implemented |
| Public testnet | Not deployed |
| Security audit | Not performed |
| Performance benchmarks | Not measured |

These are engineering milestones for the next phase.

---

## 7. Future Engineering Direction

CURRENT PROTOTYPE -> Technical Hardening -> Multi-Node Consensus -> Authentication -> Public Testnet -> Broader Workloads -> Production Network

Detailed engineering plan available upon request.

---

## 8. Why This Approach

- Demand for compute is growing — AI, scientific computing, rendering
- Distributed compute offers an alternative to centralized cloud
- Verifiable computation turns compute into a measurable, rewardable resource
- Browser-based workers eliminate installation barriers
- STARK proofs provide mathematical verification without trusted intermediaries

The prototype demonstrates the core pipeline. Engineering path to production is defined.

---

## 9. Key Points

1. A working end-to-end prototype exists. Real browser workers execute real workloads through verification, proof, block acceptance, and reward — all visible live.

2. The prototype is openly documented with its limitations. 279 passing tests (278 unit + 1 integration), public repository, honest technical disclosure.

3. The path to production is clear. Consensus, security, authentication, and testnet are defined next steps — not open research questions.

---

## Links

- GitHub: https://github.com/grgrthr/compute-chain
- Evidence: docs/VALIDATION_EVIDENCE.md
- Video: https://youtube.com/shorts/_CMj4qW8IQM
