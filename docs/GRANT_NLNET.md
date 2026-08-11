# Grant Proposal: NLnet / NGI Zero Commons Fund

## Open Infrastructure for Verifiable Distributed Computation

**Target:** EUR 40,000
**Duration:** 8 months
**License:** MIT

---

## 1. Project Summary

Compute Chain is a working open-source prototype demonstrating verifiable
distributed compute with real browser workers, independent result verification,
Merkle commitments, and STARK commitment proofs.

This grant funds hardening of the open infrastructure: multi-worker coordination,
protocol improvements, and public testnet deployment.

## 2. Problem

Most computation today happens in centralized clouds with no public verifiability.
Users cannot independently verify that results are correct. Distributed compute
lacks standardized open infrastructure for verification and reward.

## 3. Existing Landscape

Existing distributed compute projects focus on either blockchain consensus or
volunteer computing (BOINC). Few combine real-time verification, Merkle
commitments, and STARK proofs into a single open pipeline.

## 4. Compute Chain Approach

- Browser-based workers eliminate installation barriers
- Independent result verification (hash checking)
- Merkle commitment of verified data
- STARK commitment proofs for cryptographic receipts
- Live WebSocket dashboard for transparency

## 5. Current Technical State

Working prototype with:
- 279 tests, 0 failures
- Two-device validation (6 jobs, 60 rewards)
- Single-node block acceptance
- STARK commitment proofs
- See: docs/GRANT_EVIDENCE_BASE.md

## 6. Why This Matters to Open Infrastructure

Verifiable compute is a public digital commons problem. Everyone benefits from
infrastructure that can prove computation was performed correctly — from
scientific computing to public data processing.

## 7. Proposed Development

- Multi-worker coordination protocol
- Job queue with worker selection
- Protocol hardening for public deployment
- Public testnet with documentation
- Broader workload support

## 8. Milestones

| Month | Milestone |
|-------|-----------|
| 1-2 | Multi-worker protocol design and implementation |
| 3-4 | Job queue, worker selection, fault tolerance |
| 5-6 | Protocol hardening, security review |
| 7 | Public testnet deployment |
| 8 | Documentation, community guide, final report |

## 9. Deliverables

- Multi-worker coordination protocol
- Public testnet (3+ nodes)
- Technical documentation
- Community deployment guide
- Final report with test evidence

## 10. Timeline

8 months total

## 11. Budget

EUR 40,000 — development, testing, testnet operation, documentation

## 12. Open Source / Public Availability

MIT License. All code, documentation, and test evidence publicly available.

## 13. Technical Risks

- Multi-worker coordination complexity
- Network stability on public testnet
- Mitigation: incremental rollout, monitoring

## 14. Sustainability

The project is designed as open infrastructure. After grant completion, the
testnet remains publicly accessible. Community contributions are welcomed under
MIT license.

## 15. Verification Strategy

- All milestones produce public test evidence
- Test suite expansion with each milestone
- Public demo for each major deliverable
- Final validation report matching existing evidence standards

## 16. Evidence

- GitHub: https://github.com/grgrthr/compute-chain
- Evidence: docs/GRANT_EVIDENCE_BASE.md
- Video: https://youtube.com/shorts/_CMj4qW8IQM
