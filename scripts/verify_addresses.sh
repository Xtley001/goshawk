#!/usr/bin/env bash
# On-chain verification of hardcoded addresses and market IDs across all configured chains.
# NOTHING is trusted until this script prints ✓ against a synced node for that chain.
#
# Usage:
#   BASE_RPC_URL=http://127.0.0.1:8545 ./scripts/verify_addresses.sh
#   CHAIN=base BASE_RPC_URL=... ./scripts/verify_addresses.sh
#   CHAIN=arbitrum ARBITRUM_RPC_URL=... ./scripts/verify_addresses.sh

set -uo pipefail

command -v cast >/dev/null 2>&1 || { echo "cast (foundry) required"; exit 1; }

pass=0; fail=0; skipped=0
ok(){ echo "  ✓ $1"; pass=$((pass+1)); }
no(){ echo "  ✗ $1"; fail=$((fail+1)); }
skip(){ echo "  - $1 (skipped: $2)"; skipped=$((skipped+1)); }

has_code(){
  local addr="$1"
  local rpc="$2"
  if [[ -z "$addr" || "$addr" =~ ^0x0+$ ]]; then
    return 1
  fi
  local sz
  sz=$(cast codesize "$addr" --rpc-url "$rpc" 2>/dev/null || echo 0)
  [[ "${sz:-0}" =~ ^[0-9]+$ && "$sz" -gt 0 ]]
}

verify_chain_base(){
  local rpc="${BASE_RPC_URL:-}"
  if [[ -z "$rpc" ]]; then
    skip "Base" "BASE_RPC_URL not set"
    return
  fi
  echo "=== Verifying Base (Chain ID 8453) ==="
  declare -A C=(
    [BalancerVault]=0xBA12222222228d8Ba445958a75a0704d566BF2C8
    [MorphoBlue]=0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb
    [AaveAddressesProvider]=0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D
    [UniV3Factory]=0x33128a8fC17869897dcE68Ed026d694621f6FDfD
    [UniV3Router02]=0x2626664c2603336E57B271c5C0b26F421741e481
    [UniV3QuoterV2]=0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a
    [AerodromeRouter]=0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43
    [AerodromeFactory]=0x420DD381b31aEf6683db6B902084cB0FFECe40Da
    [Multicall3]=0xcA11bde05977b3631167028862bE2a173976CA11
  )
  for name in "${!C[@]}"; do
    if has_code "${C[$name]}" "$rpc"; then ok "$name ${C[$name]}"; else no "$name ${C[$name]} — NO CODE"; fi
  done

  local pool
  pool=$(cast call 0xe20fCBdBfFC4Dd138cE8b2E6FBb6CB49777ad64D "getPool()(address)" --rpc-url "$rpc" 2>/dev/null || echo "")
  local expect="0xA238Dd80C259a72e81d7e4664a9801593F98d1c5"
  if [[ "${pool,,}" == "${expect,,}" ]]; then ok "Aave getPool() == $expect"; else no "getPool()=$pool != $expect"; fi

  # Morpho markets
  local mb="0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb"
  declare -A M=(
    [WETH_USDC]=0x8793cf302b8ffd655ab97bd1c695dbd967807e8367a65cb2f4edaf1380ba1bda
    [USDC_WETH]=0x3b3769cfca57be2eaed03fcc5299c25691b77781a1e124e7a8d520eb9a7eabb5
    [cbBTC_USDC]=0xf10437266b9dd52751bd6255e15cccd0cdf5c75b58c1a3e2621130c905cd8ed9
  )
  for name in "${!M[@]}"; do
    local id="${M[$name]}"
    local out
    out=$(cast call "$mb" "idToMarketParams(bytes32)(address,address,address,address,uint256)" "$id" --rpc-url "$rpc" 2>/dev/null | head -1 || echo "")
    if [[ -n "$out" && "$out" != "0x0000000000000000000000000000000000000000" ]]; then ok "Morpho $name loanToken=$out"; else no "Morpho $name — invalid ID"; fi
  done
}

verify_generic_aave_chain(){
  local chain_name="$1"
  local rpc_var="$2"
  local aave_provider="$3"
  local rpc="${!rpc_var:-}"
  if [[ -z "$rpc" ]]; then
    skip "$chain_name" "$rpc_var not set"
    return
  fi
  echo "=== Verifying $chain_name ==="
  if [[ -z "$aave_provider" || "$aave_provider" =~ ^0x0+$ ]]; then
    no "$chain_name AaveAddressesProvider is unset or placeholder"
    return
  fi
  if has_code "$aave_provider" "$rpc"; then
    ok "$chain_name AaveAddressesProvider $aave_provider"
    local pool
    pool=$(cast call "$aave_provider" "getPool()(address)" --rpc-url "$rpc" 2>/dev/null || echo "")
    if [[ -n "$pool" && "$pool" != "0x0000000000000000000000000000000000000000" ]]; then
      ok "$chain_name resolved pool $pool"
    else
      no "$chain_name failed to resolve pool from provider"
    fi
  else
    no "$chain_name AaveAddressesProvider $aave_provider — NO CODE"
  fi
}

TARGET="${CHAIN:-all}"

if [[ "$TARGET" == "all" || "$TARGET" == "base" ]]; then
  verify_chain_base
fi

if [[ "$TARGET" == "all" || "$TARGET" == "arbitrum" ]]; then
  verify_generic_aave_chain "Arbitrum" "ARBITRUM_RPC_URL" "${ARBITRUM_AAVE_PROVIDER:-0xa97684ead0e402dC232d5A977953DF7ECBAB3CDb}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "optimism" ]]; then
  verify_generic_aave_chain "Optimism" "OPTIMISM_RPC_URL" "${OPTIMISM_AAVE_PROVIDER:-0xa97684ead0e402dC232d5A977953DF7ECBAB3CDb}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "polygon" ]]; then
  verify_generic_aave_chain "Polygon" "POLYGON_RPC_URL" "${POLYGON_AAVE_PROVIDER:-0xa97684ead0e402dC232d5A977953DF7ECBAB3CDb}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "avalanche" ]]; then
  verify_generic_aave_chain "Avalanche" "AVALANCHE_RPC_URL" "${AVALANCHE_AAVE_PROVIDER:-0xa97684ead0e402dC232d5A977953DF7ECBAB3CDb}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "bnb" ]]; then
  verify_generic_aave_chain "BNB" "BNB_RPC_URL" "${BNB_AAVE_PROVIDER:-}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "gnosis" ]]; then
  verify_generic_aave_chain "Gnosis" "GNOSIS_RPC_URL" "${GNOSIS_AAVE_PROVIDER:-0x366142503Ed309293079a5170699dA1741390504}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "ethereum" ]]; then
  verify_generic_aave_chain "Ethereum" "ETHEREUM_RPC_URL" "${ETHEREUM_AAVE_PROVIDER:-0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e}"
fi

if [[ "$TARGET" == "all" || "$TARGET" == "hyperevm" ]]; then
  rpc="${HYPEREVM_RPC_URL:-}"
  if [[ -z "$rpc" ]]; then
    skip "HyperEVM" "HYPEREVM_RPC_URL not set"
  else
    echo "=== Verifying HyperEVM ==="
    pool="${HYPERLEND_POOL:-}"
    if [[ -n "$pool" ]] && has_code "$pool" "$rpc"; then
      ok "HyperLend Pool $pool"
    else
      no "HyperLend Pool unset or no code"
    fi
  fi
fi

if [[ "$TARGET" == "all" || "$TARGET" == "plasma" ]]; then
  rpc="${PLASMA_RPC_URL:-}"
  if [[ -z "$rpc" ]]; then
    skip "Plasma" "PLASMA_RPC_URL not set"
  else
    echo "=== Verifying Plasma ==="
    verify_generic_aave_chain "Plasma" "PLASMA_RPC_URL" "${PLASMA_AAVE_PROVIDER:-}"
  fi
fi

echo
echo "──────────────────────────────────────────────"
echo "PASS: $pass    FAIL: $fail    SKIPPED: $skipped"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
exit 0
