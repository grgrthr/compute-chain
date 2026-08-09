# Investor Demo — Step by Step

## Requirements
- Device 1: Server (Linux/Mac/Windows)
- Device 2: Modern browser (phone/tablet/laptop)
- Same network

## 1. Start Server
Shows: `COMPUTE CHAIN — 0.1.0-demo`

## 2. Connect Worker
Device 2: `http://SERVER_IP:3000/worker` → **Join Network**

## 3. Open Dashboard
`http://SERVER_IP:3000/investor` → Marketplace section

## 4. Submit Job
Upload image → **Submit Job**

## 5. Pipeline
Submitted → Assigned → Progress → Verified → Merkle → Proof → Block → Reward → Finalized

## 6. Repeat 3x
Expected: Block #1/#2/#3, +10 each, Worker: 3 Done, 30 Rewards, Idle

## Proof Meaning
Commitment Proof — proves verified data bound to Merkle root.
Does NOT prove JavaScript execution.
