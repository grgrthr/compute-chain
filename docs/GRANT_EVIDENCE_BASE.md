# Compute Chain — Grant Evidence Base

## Repository

- URL: https://github.com/grgrthr/compute-chain
- License: MIT — Copyright (c) 2026 Ismail Abdo
- Language: Rust (92.7%), HTML (6%)

## Evidence Capture

- Evidence Capture Commit: a9264c3 (original demo validation)
- Current Repository HEAD: 1565279

## Build Status

- cargo check: PASS
- cargo build: PASS
- cargo test: 278 unit + 1 integration = 279 total, 0 failed

## Demo Validation

- Two real devices (laptop server + phone worker)
- Browser worker connects via WebSocket
- 6 consecutive jobs completed without refresh
- 6 rewards distributed, total 60 tokens
- Worker returned to Idle after each job
- Dashboard updated live (WebSocket, no refresh)

## Architecture State

| Component | Current Status |
|-----------|---------------|
| Worker execution | Real browser worker (thumbnail, hash, CSV) |
| Verification | Independent hash verification |
| Merkle | SHA-256 commitment tree |
| STARK | Commitment proof (not JS execution proof) |
| Block acceptance | Single-node |
| Consensus | Prototype implementations only |
| P2P | Local network only |
| Authentication | Not implemented |
| Testnet | Not deployed |

## Explicit Limitations

- NOT a production blockchain
- NOT decentralized consensus
- STARK proves commitment integrity, not browser JS execution
- No persistent key management
- No security audit
- No performance benchmarks

## Evidence Files

- demo_evidence/build.txt — cargo build output
- demo_evidence/check.txt — cargo check output
- demo_evidence/test.txt — 279 tests passed
- demo_evidence/server.txt — server startup log
- docs/VALIDATION_EVIDENCE.md — full validation report
- docs/EVIDENCE_INDEX.md — claim-to-evidence map

## Video

https://youtube.com/shorts/_CMj4qW8IQM
