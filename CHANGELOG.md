# Changelog

Documented release history and protocol migrations for Goshawk.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-09-05

### Added
- **Ethereum Mainnet Liquidation Engine:** Dedicated execution system targeting Ethereum Mainnet (Chain ID 1).
- **Lending Adapters:** Implementation of 6 lending protocols (Aave V3, Morpho Blue, Spark, Fluid, Compound V3, Euler V2).
- **Flash Loan Registry:** Integrated support for Morpho Blue (0%), Balancer V2 (0%), Spark DSS Flash (0%), and Aave V3 (0.05% fee backstop).
- **Swap Venue Registry:** Integrated quote and swap execution across Uniswap V3, Curve, and Balancer.
- **Dual Independent Economic Gates:** Gate 1 (dynamic gas cost floor + flat $750 hurdle) and Gate 2 (protocol break-even debt + min clip).
- **Aave V3 SVR Exclusion:** Automatic exclusion of Soft-Volatile Reserves (tBTC, AAVE) to prevent unhedged spread losses.
- **Gas-Regime Scaling:** Normal vs Spike (>60 gwei) regime scaling for Aave V3 / WBTC liquidations.
- **Private Builder Pipeline:** Direct submission to Flashbots Relay, Titan Builder, and Beaver Build with 3-failure failover.
- **Smart Contracts:** Deployed `FlashExecutor.sol` and `ExecutorBase.sol` with strict `block.chainid == 1` enforcement and 5 flash callback shapes.
- **On-Chain Address Verification:** Automated validator (`scripts/verify_addresses.sh`) asserting live bytecode and aggregators for all sourced contracts.

### Changed
- **Architecture Migration:** Rebuilt and migrated codebase from legacy Corvus multi-chain architecture to single-chain Ethereum Mainnet focus.
- **Open Gap Enforcement:** All unverified contract addresses remain explicit zero-placeholders held in disabled mode with zero address fabrication.

### Removed
- Removed all non-Ethereum chain adapters, DEX modules, and configurations (Base, HyperEVM, Plasma, Arbitrum, Optimism, Polygon, Avalanche, BNB, Gnosis).
