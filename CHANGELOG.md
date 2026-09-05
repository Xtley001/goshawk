# Changelog

*All notable changes to the Goshawk liquidation engine.*

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-09-05 — Goshawk Ethereum Rebuild

### Changed
- **Project Rebrand:** Complete rebuild of Corvus into **Goshawk** — an Ethereum-mainnet-only (Chain ID 1) liquidation engine.
- **Single-Chain Focus:** Reduced scope exclusively to Ethereum Mainnet (Chain ID 1). Deleted all non-Ethereum chain adapters, DEX modules, and configurations (Base, HyperEVM, Plasma, Arbitrum, Optimism, Polygon, Avalanche, BNB, Gnosis).
- **Execution Contract:** Rewrote `ExecutorBase.sol` and `FlashExecutor.sol` with compile-time / runtime `block.chainid == 1` enforcement, support for 6 lending protocols (Aave V3, Morpho Blue, Spark, Fluid, Compound V3, Euler V2), and 5 flash loan callbacks (`onMorphoFlashLoan`, `receiveFlashLoan`, `onFlashLoan`, `executeOperation`, `uniswapV3FlashCallback`).
- **Zero Address Fabrication Rule:** All unsourced protocol contracts and oracle feeds remain explicit `TODO(GAP)` entries (value `""` in Rust, `address(0)` in Solidity) with adapters held disabled until verified against live on-chain state.
- **Dual Independent Economic Gates:** Implemented independent Gate 1 (profit >= dynamic gas floor and flat $750 floor) and Gate 2 (debt >= protocol break-even debt and min clip).
- **Break-Even & Regime Scaling:** Integrated per-protocol break-even debt figures ($900 Morpho Blue, $1,200 Compound V3, $2,333.33 Spark, $2,700 Aave V3) and Aave V3/WBTC Normal vs Spike gas-regime scaling ($5,000 clip / $770 debt vs $25,000 clip / $5,600 debt).
- **Aave V3 SVR Exclusion:** Excluded Soft-Volatile Reserves (tBTC, AAVE) on Ethereum Mainnet.
- **Flash Loan Precedence:** Aligned flash loan priority order to 07 §7.2: Morpho Blue (0%), Balancer V2 (0%), Spark DSS Flash (0%), Aave V3 (0.05% backstop).
- **Private Submission Relays:** Configured direct Ethereum builder relays (Flashbots Relay, Titan Builder, Beaver Build) with automatic failover.
- **Verification Tooling:** Rewrote `scripts/verify_addresses.sh` to verify all sourced Ethereum contracts and feeds on-chain while explicitly listing all 17 gaps.

## [1.1.0] - 2026-09-03 (Corvus Historical)

### Added
- **Multi-Chain Architecture:** Expanded from single-chain Base operation to 10 EVM networks.
- **Dynamic Risk Engine:**
  - Dynamic minimum liquidation profit hurdle derived from live gas costs and opportunity cost floor.
  - Realized AMM slippage calculation derived from virtual pool reserves.
  - Volatility-widened health factor gate adjusting monitoring thresholds to rolling return variance.
  - Adaptive cost-weighted circuit breakers scaling penalties by realized gas expenditure.
  - Parallel live flash quoting across Balancer, Morpho, Aave, and HyperLend.
- **Private Submission Everywhere:** Defaulted private builder routing across EVM chains.
- **Unified Contract Architecture:** Deployed `ExecutorBase.sol` with four isolated flash callback branches.

### Removed
- Deprecated legacy S1 cross-DEX arb, S2 triangular arb, S4 JIT liquidity, and S5 rate arbitrage modules.
- Removed hardcoded static thresholds.

## [1.0.0] - 2026-08-15 (Corvus Historical)

### Added
- Initial release of Corvus liquidation engine for Base mainnet.
- Integration with Aave V3 and Morpho Blue lending pools.
- Local REVM fork simulation and IPC connectivity.
