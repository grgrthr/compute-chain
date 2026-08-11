# Compute Chain — Verifiable Compute Prototype

**Version:** 0.1.0-demo
**Status:** Working prototype

## What This Is
Compute Chain demonstrates verifiable distributed compute: a real browser worker
executes a job, the result is verified, Merkle-committed, STARK-proven, recorded
in a block, and rewarded — live on a dashboard.

**This is NOT a production blockchain.** It is a working prototype.

## What Works
- Real browser worker (mobile/desktop)
- Job submission with file upload
- Real task execution (thumbnail, hash, CSV)
- Live WebSocket progress updates
- Result verification + Merkle commitment
- STARK commitment proof + independent verification
- Block acceptance (single-node) + reward distribution
- Live 10-section dashboard, 3 consecutive jobs without refresh

## What Does NOT Work Yet
- Production decentralized consensus
- P2P hardening, authentication, persistent keys, security hardening
- Broad workload types, production testnet

## Quick Start
Device 1: `cargo build && ./target/debug/compute_chain`
Device 2: Open `http://SERVER_IP:3000/worker` → Join Network
Dashboard: `http://SERVER_IP:3000/investor`

## License
MIT License — Copyright (c) 2026 Ismail Abdo
