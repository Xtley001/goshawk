# Changelog

*All notable changes to the Corvus liquidation engine.*

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-09-03

### Added
- **Multi-Chain Architecture:** Expanded from single-chain Base operation to 10 EVM networks (Base, HyperEVM, Arbitrum, Ethereum, Optimism, Polygon, Avalanche, BNB Chain, Gnosis, Plasma).
- **Dynamic Risk Engine:**
  - Dynamic minimum liquidation profit hurdle derived from live gas costs and opportunity cost floor.
  - Realized AMM slippage calculation derived from virtual pool reserves.
  - Volatility-widened health factor gate adjusting monitoring thresholds to rolling return variance.
  - Adaptive cost-weighted circuit breakers scaling penalties by realized gas expenditure.
  - Parallel live flash quoting across Balancer, Morpho, Aave, and HyperLend.
- **Private Submission Everywhere:** Defaulted private builder routing across Ethereum (Flashbots Protect), Arbitrum (Timeboost), Polygon (Bor Private), Base (Alchemy MEV-protected RPC), and BNB Chain (bloXroute).
- **Unified Contract Architecture:** Deployed `ExecutorBase.sol` with four isolated flash callback branches (Balancer, Morpho Blue, Aave V3, HyperLend Native).
- **Documentation Suite:** Authored DeFi-grade documentation including formal protocol whitepaper (`docs/whitepaper.md`) and deep architecture reference (`docs/ARCHITECTURE.md`).

### Removed
- Deprecated legacy S1 cross-DEX arb, S2 triangular arb, S4 JIT liquidity, and S5 rate arbitrage modules in favor of dedicated liquidation execution.
- Removed hardcoded static thresholds (`$50` flat profit floor, `1.8%` static swap haircut, static flash priority order).
- Removed internal deployment scratchpads (`TODO.md`).

## [1.0.0] - 2026-08-15

### Added
- Initial release of Corvus liquidation engine for Base mainnet.
- Integration with Aave V3 and Morpho Blue lending pools.
- Local REVM fork simulation and IPC connectivity.
