#!/usr/bin/env bash
# Goshawk v1.0 — contract deployment script
# Ethereum Mainnet (Chain ID: 1)
# Usage: ./scripts/deploy.sh
#
# Required environment variables (set in your shell or sourced .env):
#   DEPLOYER_PRIVATE_KEY    — deployer wallet private key (uint256)
#   EXECUTOR_ADDRESS        — hot wallet EOA for flash loan submission
#   COLD_WALLET_ADDRESS     — immutable sweep destination (baked into contract)
#   ETHERSCAN_API_KEY       — for Etherscan source verification
#   ETH_RPC_URL             — Ethereum Mainnet RPC URL (defaults to http://127.0.0.1:8545)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
CONTRACTS_DIR="$REPO_ROOT/contracts"
RPC_URL="${ETH_RPC_URL:-${ETHEREUM_RPC_URL:-http://127.0.0.1:8545}}"

echo "╔══════════════════════════════════════╗"
echo "║    Goshawk Contract Deploy           ║"
echo "║    Ethereum Mainnet (Chain ID: 1)    ║"
echo "╚══════════════════════════════════════╝"

# Validate required env vars
: "${DEPLOYER_PRIVATE_KEY:?Must set DEPLOYER_PRIVATE_KEY}"
: "${EXECUTOR_ADDRESS:?Must set EXECUTOR_ADDRESS}"
: "${COLD_WALLET_ADDRESS:?Must set COLD_WALLET_ADDRESS}"
: "${ETHERSCAN_API_KEY:?Must set ETHERSCAN_API_KEY}"

# Create position store directory for persistence
echo "Ensuring position store directory exists..."
sudo mkdir -p /var/lib/goshawk
sudo chown "$(id -u):$(id -g)" /var/lib/goshawk
echo "  /var/lib/goshawk ready"

# Verify node is connected and on Chain ID 1
CHAIN_ID=$(cast chain-id --rpc-url "$RPC_URL" 2>/dev/null || echo "")
if [[ "$CHAIN_ID" != "1" ]]; then
    echo "ERROR: RPC at $RPC_URL returned chain ID '$CHAIN_ID', expected 1 (Ethereum Mainnet)"
    exit 1
fi

BLOCK=$(cast block-number --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
echo "Ethereum node connected at block: $BLOCK (Chain ID: 1)"

# Build and test
cd "$CONTRACTS_DIR"
echo "Building contracts..."
forge build --silent

echo "Running fork tests (using $RPC_URL)..."
ETH_RPC_URL="$RPC_URL" forge test --fork-url "$RPC_URL" -vv
echo "All tests passed."

# Deploy FlashExecutor
echo "Deploying FlashExecutor to Ethereum Mainnet..."
DEPLOY_OUTPUT=$(forge script script/Deploy.s.sol \
  --rpc-url "$RPC_URL" \
  --broadcast \
  --verify \
  --etherscan-api-key "$ETHERSCAN_API_KEY" \
  -vvvv 2>&1)

echo "$DEPLOY_OUTPUT"

# Extract deployed addresses
EXECUTOR_ADDR=$(echo "$DEPLOY_OUTPUT" | grep "FlashExecutor:" | awk '{print $2}')
AAVE_POOL=$(echo "$DEPLOY_OUTPUT" | grep "Aave V3 Pool (resolved):" | awk '{print $5}')

if [[ -z "$EXECUTOR_ADDR" ]]; then
    echo "ERROR: Could not extract deployed FlashExecutor address from output"
    exit 1
fi

echo ""
echo "═══════════════════════════════════════════════"
echo "DEPLOYMENT SUCCESSFUL (Ethereum Mainnet)"
echo "FlashExecutor:          $EXECUTOR_ADDR"
echo "Aave V3 Pool (resolved):$AAVE_POOL"
echo "Cold Wallet:            $COLD_WALLET_ADDRESS"
echo "═══════════════════════════════════════════════"
echo ""
echo "Next steps:"
echo "  1. Add to your .env / config:"
echo "     GOSHAWK_FLASH_EXECUTOR_ADDRESS=$EXECUTOR_ADDR"
echo ""
echo "  2. Verify Aave pool resolved correctly:"
echo "     cast call $EXECUTOR_ADDR 'AAVE_V3_POOL()(address)' --rpc-url $RPC_URL"
echo ""
echo "  3. Verify contract addresses before live execution:"
echo "     ETH_RPC_URL=$RPC_URL ./scripts/verify_addresses.sh"
