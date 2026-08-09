#!/bin/bash
# Compute Chain - Smoke Test
# Test: send tx → mine block → check chain → mint nft → read nft

BASE="http://localhost:3099"
PASS=0
FAIL=0

echo "╔══════════════════════════════════════════╗"
echo "║     Compute Chain - Smoke Test          ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# 1. Health
echo "1. Health Check..."
RES=$(curl -s $BASE/health 2>/dev/null)
if echo "$RES" | grep -q '"api":true'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

# 2. Send Transaction
echo "2. Send Transaction..."
RES=$(curl -s -X POST $BASE/tx/send \
    -H "Content-Type: application/json" \
    -d '{"from":"validator1","to":"smoketest","amount":1}' 2>/dev/null)
if echo "$RES" | grep -q '"status":"success"'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

# 3. Mine Block
echo "3. Mine Block..."
RES=$(curl -s -X POST $BASE/block/mine \
    -H "Content-Type: application/json" \
    -d '{"validator_id":"smoketest","program":[{"opcode":"MOV","params":[0,1]},{"opcode":"HALT","params":[]}]}' 2>/dev/null)
if echo "$RES" | grep -q '"status":"block_mined"'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

# 4. Check Chain
echo "4. Check Chain..."
RES=$(curl -s $BASE/chain 2>/dev/null)
if echo "$RES" | grep -q '"height"'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

# 5. Mint NFT
echo "5. Mint NFT..."
RES=$(curl -s -X POST $BASE/nft/mint \
    -H "Content-Type: application/json" \
    -d '{"owner":"smoketest","name":"SmokeNFT","data":"test"}' 2>/dev/null)
if echo "$RES" | grep -q '"status":"minted"'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

# 6. List NFTs
echo "6. List NFTs..."
RES=$(curl -s $BASE/nfts 2>/dev/null)
if echo "$RES" | grep -q '"nfts"'; then
    echo "   ✅ PASS"
    PASS=$((PASS+1))
else
    echo "   ❌ FAIL"
    FAIL=$((FAIL+1))
fi

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║   PASS: $PASS   FAIL: $FAIL                  ║"
if [ $FAIL -eq 0 ]; then
    echo "║   RESULT: ALL TESTS PASSED ✅           ║"
else
    echo "║   RESULT: SOME TESTS FAILED ❌          ║"
fi
echo "╚══════════════════════════════════════════╝"

exit $FAIL
