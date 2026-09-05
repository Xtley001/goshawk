#!/usr/bin/env bash
# sweep.sh — Goshawk profit sweep script (Ethereum Mainnet)
# LOW-07 fix: confirmation prompt before irreversible sweep operation.
set -euo pipefail

# ── Env validation ─────────────────────────────────────────────────────────────
HOT_WALLET="${GOSHAWK_HOT_WALLET:-}"
COLD_WALLET="${GOSHAWK_COLD_WALLET:-}"
FLASH_EXECUTOR="${GOSHAWK_FLASH_EXECUTOR:-}"
HOT_PRIVATE_KEY="${GOSHAWK_HOT_PRIVATE_KEY:-}"
RPC_URL="${ETH_RPC_URL:-${ETHEREUM_RPC_URL:-}}"

: "${HOT_WALLET:?GOSHAWK_HOT_WALLET env var not set}"
: "${COLD_WALLET:?GOSHAWK_COLD_WALLET env var not set}"
: "${FLASH_EXECUTOR:?GOSHAWK_FLASH_EXECUTOR env var not set}"
: "${HOT_PRIVATE_KEY:?GOSHAWK_HOT_PRIVATE_KEY env var not set}"
: "${RPC_URL:?ETH_RPC_URL env var not set}"

TOKEN="${1:-}"
if [[ -z "$TOKEN" ]]; then
    echo "Usage: $0 <TOKEN_ADDRESS>"
    echo "Example: $0 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48   # USDC (Ethereum Mainnet)"
    exit 1
fi

# ── Query balance ──────────────────────────────────────────────────────────────
echo "Querying balance of $TOKEN at $FLASH_EXECUTOR..."
BALANCE=$(cast call "$TOKEN" "balanceOf(address)(uint256)" "$FLASH_EXECUTOR" \
    --rpc-url "$RPC_URL" 2>/dev/null || echo "0")

if [[ "$BALANCE" == "0" || -z "$BALANCE" ]]; then
    echo "Balance is zero. Nothing to sweep."
    exit 0
fi

# ── LOW-07: Confirmation prompt — prevents fat-finger sweeps ──────────────────
echo ""
echo "┌─────────────────────────────────────────────────────────────┐"
echo "│  GOSHAWK SWEEP CONFIRMATION (Ethereum Mainnet)             │"
echo "│                                                             │"
printf "│  Token:    %-48s│\n" "$TOKEN"
printf "│  Balance:  %-48s│\n" "$BALANCE (raw)"
printf "│  From:     %-48s│\n" "$FLASH_EXECUTOR"
printf "│  To:       %-48s│\n" "$COLD_WALLET"
echo "│                                                             │"
echo "│  ⚠  This action is IRREVERSIBLE on Ethereum mainnet.       │"
echo "└─────────────────────────────────────────────────────────────┘"
echo ""
read -r -p "Confirm sweep? [y/N] " CONFIRM
if [[ "${CONFIRM,,}" != "y" ]]; then
    echo "Aborted. No transaction submitted."
    exit 0
fi

# ── Secondary confirmation for large balances (>$10K equivalent) ──────────────
if (( BALANCE > 10_000_000_000 )); then   # >$10K if USDC (6 dec)
    echo ""
    echo "⚠  Large balance detected. Type 'CONFIRM' in caps to proceed:"
    read -r CAPS_CONFIRM
    if [[ "$CAPS_CONFIRM" != "CONFIRM" ]]; then
        echo "Aborted."
        exit 0
    fi
fi

# ── Execute sweep via cast send ────────────────────────────────────────────────
echo "Submitting sweep transaction to Ethereum Mainnet..."
TX_HASH=$(cast send "$FLASH_EXECUTOR" \
    "sweep(address,address)" "$TOKEN" "$COLD_WALLET" \
    --private-key "$HOT_PRIVATE_KEY" \
    --rpc-url "$RPC_URL" \
    --json 2>&1 | jq -r '.transactionHash // empty')

if [[ -z "$TX_HASH" ]]; then
    echo "ERROR: Transaction submission failed. Check RPC connection and wallet balance."
    exit 1
fi

echo ""
echo "✓  Sweep submitted: $TX_HASH"
echo "   Track: https://etherscan.io/tx/$TX_HASH"

# ── Wait for confirmation ──────────────────────────────────────────────────────
echo "Waiting for confirmation..."
cast receipt "$TX_HASH" --rpc-url "$RPC_URL" --confirmations 3 > /dev/null 2>&1 \
    && echo "✓  Confirmed (3 blocks)" \
    || echo "⚠  Could not confirm — check etherscan.io manually"
