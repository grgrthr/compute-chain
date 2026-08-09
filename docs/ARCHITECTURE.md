# Compute Chain — Architecture

## Flow
Dashboard → API → Job → Worker WS → Browser Worker → Result
→ Verification → Merkle → STARK Proof → quick_verify
→ Block (single-node) → Reward → Live Dashboard

## Components
| Layer | Tech |
|-------|------|
| API | axum 0.7 |
| VM | 18-instruction CPU |
| Merkle | SHA-256 tree |
| STARK | AIR + FRI |
| P2P | libp2p 0.53 |
| Token | TokenEngine |
| Dashboard | Vanilla JS + WS |

## Consensus
Prototype PBFT/PoS/PoW exist. Current demo uses single-node block acceptance.
