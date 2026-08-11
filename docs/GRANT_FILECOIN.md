# Grant Proposal: Filecoin Foundation Open Grant

## Verifiable Compute Integration for Decentralized Data Infrastructure

**Target:** $30,000
**Duration:** 6 months
**License:** MIT

---

## 1. Project Summary

Compute Chain is a working prototype demonstrating verifiable distributed compute.
Real browser workers execute useful workloads, results are independently verified,
Merkle-committed, STARK-proven, and rewarded.

This grant funds integration of verifiable compute infrastructure with the Filecoin
ecosystem, enabling verifiable computation on data stored in decentralized storage.

## 2. Problem

Filecoin provides decentralized storage. However, computation on stored data
currently lacks a standardized verifiable layer. Users retrieving data cannot
independently verify that computations performed on that data are correct.

## 3. Why Verifiable Compute

Verifiable compute produces cryptographically-verifiable receipts proving that a
specific computation was performed on specific data, producing a specific output.
This is complementary to Filecoin's storage proofs.

## 4. Current Working Prototype

Compute Chain has a working two-device prototype:
- Real browser workers execute workloads
- Results are independently verified
- Merkle commitments bind verified data
- STARK commitment proofs provide cryptographic receipts
- 279 tests, 0 failures
- 6-job manual validation, 60 rewards

## 5. Existing Evidence

See: docs/GRANT_EVIDENCE_BASE.md

## 6. Why Filecoin

Filecoin stores data. Compute Chain verifies computation. Together they enable
a complete verifiable storage + compute pipeline. A Filecoin user could store data
and later prove that specific computations were correctly performed on that data.

## 7. Proposed Integration

- Define a standard format for compute-on-Filecoin-data jobs
- Integrate Filecoin CID (content identifiers) into Compute Chain job definitions
- Enable workers to fetch data from Filecoin/IPFS, compute, and submit verifiable results
- Store compute proofs alongside Filecoin data for auditability

## 8. Technical Approach

- Extend job schema to reference Filecoin CIDs
- Add IPFS/Filecoin data fetching to worker dispatch
- Verify that input data matches the referenced CID before computation
- Include CID in Merkle commitment and STARK proof

## 9. Milestones

| Month | Milestone |
|-------|-----------|
| 1-2 | CID-aware job schema, data fetching prototype |
| 3-4 | CID verification in proof pipeline |
| 5 | End-to-end Filecoin-to-compute demonstration |
| 6 | Documentation, tests, public demo |

## 10. Deliverables

- CID-integrated job submission and verification
- Filecoin data fetching worker module
- End-to-end demo: store on Filecoin, compute, verify
- Technical documentation
- Public test evidence

## 11. Timeline

6 months total

## 12. Budget

$30,000 — development, testing, documentation

## 13. Open Source Commitment

MIT License. All code publicly available on GitHub.

## 14. Risks

- Filecoin API changes during development
- Network latency for large data fetches
- Mitigation: pinning services, caching layer

## 15. Success Criteria

- Filecoin CID can be submitted as job input
- Worker fetches data from IPFS/Filecoin
- Proof verifies both data origin and computation result
- Public demonstration recorded

## 16. Links

- GitHub: https://github.com/grgrthr/compute-chain
- Evidence: docs/GRANT_EVIDENCE_BASE.md
- Video: https://youtube.com/shorts/_CMj4qW8IQM
