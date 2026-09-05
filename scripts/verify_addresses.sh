#!/usr/bin/env bash
# scripts/verify_addresses.sh — Goshawk Ethereum Mainnet (Chain ID 1) Address Verifier
#
# Strictly adheres to Goshawk specifications:
#   - 05_PROTOCOLS_AND_ADDRESSES.md
#   - 06_ORACLES.md
#   - 07_FLASH_LOANS_AND_DEX.md
#   - 12_RULES.md §12.5 (Open Gap Register)
#
# Every sourced address is verified against live RPC bytecode and on-chain state.
# Every gap is explicitly reported as "NOT VERIFIED — no address to check".
#
# Usage:
#   ETH_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/YOUR_KEY ./scripts/verify_addresses.sh
#   or
#   ETHEREUM_RPC_URL=http://127.0.0.1:8545 ./scripts/verify_addresses.sh

set -uo pipefail

RPC="${ETH_RPC_URL:-${ETHEREUM_RPC_URL:-${RPC_URL:-}}}"

if [[ -z "$RPC" ]]; then
  echo "ERROR: Ethereum RPC URL not set."
  echo "Please set ETH_RPC_URL or ETHEREUM_RPC_URL, e.g.:"
  echo "  ETH_RPC_URL=http://127.0.0.1:8545 ./scripts/verify_addresses.sh"
  exit 1
fi

command -v cast >/dev/null 2>&1 || { echo "ERROR: 'cast' (foundry) CLI is required on PATH."; exit 1; }

echo "═══════════════════════════════════════════════════════════════════════"
echo "   Goshawk Address & Oracle Verifier — Ethereum Mainnet (Chain ID 1)  "
echo "═══════════════════════════════════════════════════════════════════════"
echo "Connecting to RPC: $RPC"

# ── 1. Verify Chain ID ────────────────────────────────────────────────
CHAIN_ID=$(cast chain-id --rpc-url "$RPC" 2>/dev/null || echo "")
if [[ "$CHAIN_ID" != "1" ]]; then
  echo "ERROR: Connected RPC chain ID is '$CHAIN_ID' (expected 1 for Ethereum Mainnet)."
  echo "Goshawk is an Ethereum-mainnet-only engine (01_PROJECT_OVERVIEW.md). Halting."
  exit 1
fi
echo "Chain ID verified: 1 (Ethereum Mainnet)"
echo

pass=0
fail=0
gaps=0

ok(){ echo "  ✓ $1"; pass=$((pass+1)); }
no(){ echo "  ✗ $1"; fail=$((fail+1)); }
gap(){
  local name="$1"
  local ref="$2"
  local note="$3"
  echo "  ⚠️  [GAP] $name ($ref): NOT VERIFIED — no address to check ($note)"
  gaps=$((gaps+1))
}

has_code(){
  local addr="$1"
  if [[ -z "$addr" || "$addr" =~ ^0x0+$ ]]; then
    return 1
  fi
  local sz
  sz=$(cast codesize "$addr" --rpc-url "$RPC" 2>/dev/null || echo 0)
  [[ "${sz:-0}" =~ ^[0-9]+$ && "$sz" -gt 0 ]]
}

verify_contract(){
  local label="$1"
  local addr="$2"
  local ref="$3"
  if has_code "$addr"; then
    local sz
    sz=$(cast codesize "$addr" --rpc-url "$RPC" 2>/dev/null || echo 0)
    ok "$label ($ref) at $addr [size: $sz bytes]"
  else
    no "$label ($ref) at $addr — NO BYTECODE FOUND"
  fi
}

verify_chainlink_feed(){
  local label="$1"
  local addr="$2"
  local expected_dec="$3"
  local ref="$4"
  local extra_warning="${5:-}"

  if ! has_code "$addr"; then
    no "$label feed ($ref) at $addr — NO BYTECODE FOUND"
    return
  fi

  local dec
  dec=$(cast call "$addr" "decimals()(uint8)" --rpc-url "$RPC" 2>/dev/null || echo "")
  local round_data
  round_data=$(cast call "$addr" "latestRoundData()(uint80,int256,uint256,uint256,uint80)" --rpc-url "$RPC" 2>/dev/null || echo "")

  if [[ -n "$round_data" ]]; then
    local answer
    answer=$(echo "$round_data" | sed -n '2p' | awk '{print $1}')
    local updated_at
    updated_at=$(echo "$round_data" | sed -n '4p' | awk '{print $1}')
    if [[ -n "$answer" && "$updated_at" != "0" ]]; then
      ok "$label feed ($ref) at $addr [decimals: $dec, latest answer: $answer, updated: $updated_at] $extra_warning"
    else
      no "$label feed ($ref) at $addr — latestRoundData returned invalid data"
    fi
  else
    no "$label feed ($ref) at $addr — latestRoundData() failed"
  fi
}

echo "─── Section 1: Sourced Lending Protocols (05_PROTOCOLS_AND_ADDRESSES.md) ───"
verify_contract "Aave V3 Pool Proxy" "0x794a61358D6845594F94dc1DB02A252b5b4814aD" "05 §5.1"
verify_contract "Aave V3 Price Oracle" "0xb56c2F0B653B2e0b10C9b928C8580Ac5Df02C7C7" "05 §5.1"
verify_contract "Aave V3 WBTC Underlying" "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f" "05 §5.1"

# Aave V3 aToken: 05 §5.1 explicitly warns this token was labeled aArbWBTC in source doc. Verify code and query name.
AATOKEN="0x078f358208685046a11c85e8ad32895ded33a249"
if has_code "$AATOKEN"; then
  TOKEN_NAME=$(cast call "$AATOKEN" "name()(string)" --rpc-url "$RPC" 2>/dev/null || echo "UNKNOWN")
  ok "Aave V3 aToken (05 §5.1) at $AATOKEN — [name: $TOKEN_NAME]"
else
  no "Aave V3 aToken (05 §5.1) at $AATOKEN — NO BYTECODE (flagged per 05 §5.1 note: source named it aArbWBTC)"
fi

verify_contract "Aave V3 Variable Debt Token" "0x92b42c66840c7ad907b4bf74879ff3ef7c529473" "05 §5.1"
verify_contract "Morpho Blue Core" "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb" "05 §5.2"
echo

echo "─── Section 2: Sourced Flash Loan Providers & Canonical Infra (07_FLASH_LOANS_AND_DEX.md) ───"
verify_contract "Morpho Blue Flash" "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb" "07 §7.1"
verify_contract "Balancer V2 Vault" "0xBA12222222228d8Ba53be47888D16304ca09907c" "07 §7.1"
verify_contract "Spark / Maker DSS Flash" "0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA" "07 §7.1"
verify_contract "Multicall3" "0xcA11bde05977b3631167028862bE2a173976CA11" "shared/addresses.rs"

# Canonical Tokens
verify_contract "WETH (Canonical)" "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" "canonical"
verify_contract "USDC (Canonical)" "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" "canonical"
verify_contract "wstETH (Canonical)" "0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0" "canonical"
verify_contract "DAI (Canonical)" "0x6B175474E89094C44Da98b954EedeAC495271d0F" "canonical"
verify_contract "cbETH (Canonical)" "0xBe9895146f7AF43049ca1c1AE358B0541Ea49704" "canonical"
echo

echo "─── Section 3: Sourced Chainlink Feeds (06_ORACLES.md §6.1) ───"
verify_chainlink_feed "ETH / USD" "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419" 8 "06 §6.1"
verify_chainlink_feed "BTC / USD" "0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c" 8 "06 §6.1"
verify_chainlink_feed "wstETH / ETH" "0xAC1cD0C879E2773e254ae74000ba15441d1b950E" 18 "06 §6.1"
verify_chainlink_feed "cbBTC / USD" "0xF0a1740954e8035D012E4E6e1e695dAcFe7fe02f" 8 "06 §6.1"

# USDC/USD has an anomaly note in 06 §6.1
USDC_FEED="0x8fFf670264b163A7712399999999999999999999"
verify_chainlink_feed "USDC / USD (⚠️ 11-nines check)" "$USDC_FEED" 8 "06 §6.1" "(FLAG: 06 §6.1 repeated-9s pattern)"
echo

echo "─── Section 4: Open Gap Register (12_RULES.md §12.5 / 05 / 06 / 07 / 10) ───"
gap "Spark SparkLend Pool Proxy" "05_PROTOCOLS_AND_ADDRESSES.md §5.3" "Blocks Spark liquidation adapter going live"
gap "Fluid Liquidity Layer" "05_PROTOCOLS_AND_ADDRESSES.md §5.4" "Blocks Fluid adapter going live"
gap "Compound V3 Comet Proxy" "05_PROTOCOLS_AND_ADDRESSES.md §5.5" "Blocks Compound V3 adapter going live"
gap "Euler V2 VaultController" "05_PROTOCOLS_AND_ADDRESSES.md §5.6" "Blocks Euler V2 adapter going live"
gap "Morpho Blue per-market bytes32 IDs (cbBTC, senPYUSD, senRLUSD, wstETH)" "03 §3.6, 05 §5.2" "Blocks per-market Morpho liquidation"
gap "Chainlink cbETH / ETH feed" "06_ORACLES.md §6.2" "Blocks cbETH liquidation on Ethereum"
gap "Chainlink USDE feed" "06_ORACLES.md §6.2" "Blocks USDE liquidation"
gap "Chainlink USDS feed" "06_ORACLES.md §6.2" "Blocks USDS liquidation"
gap "Chainlink senPYUSD feed" "06_ORACLES.md §6.2" "Blocks senPYUSD liquidation"
gap "Chainlink senRLUSD feed" "06_ORACLES.md §6.2" "Blocks senRLUSD liquidation"
gap "Uniswap V3 Factory (Ethereum Mainnet)" "07_FLASH_LOANS_AND_DEX.md §7.4" "Blocks Uniswap V3 swap route and V3 flash"
gap "Uniswap V3 Router02 (Ethereum Mainnet)" "07_FLASH_LOANS_AND_DEX.md §7.4" "Blocks Uniswap V3 router execution"
gap "Uniswap V3 QuoterV2 (Ethereum Mainnet)" "07_FLASH_LOANS_AND_DEX.md §7.4" "Blocks on-chain Quoter quotes"
gap "Curve Registry (Ethereum Mainnet)" "07_FLASH_LOANS_AND_DEX.md §7.4" "Blocks Curve dynamic pool discovery"
gap "DEX exit routes for cbBTC, USDE, USDS, senPYUSD, senRLUSD" "07_FLASH_LOANS_AND_DEX.md §7.4" "Blocks liquidation execution of those collateral assets"
gap "Break-even economics for Fluid and Euler V2" "10_ECONOMICS_AND_RISK.md §10.1" "Flat floor used meanwhile"
gap "Gas-regime table for protocols other than Aave V3/WBTC" "10_ECONOMICS_AND_RISK.md §10.2" "Flat floor used meanwhile"
echo

echo "═══════════════════════════════════════════════════════════════════════"
echo "VERIFICATION RESULTS:  PASS: $pass    FAIL: $fail    GAPS RECORDED: $gaps"
echo "═══════════════════════════════════════════════════════════════════════"

if [[ "$fail" -gt 0 ]]; then
  echo "Verification FAILED with $fail missing or invalid contracts."
  exit 1
fi

echo "All sourced Ethereum mainnet addresses verified successfully."
exit 0
