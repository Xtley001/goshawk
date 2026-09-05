# Corvus

*Autonomous multi-chain protocol liquidation engine with dynamic risk modeling and private submission.*

[![CI](https://img.shields.io/github/actions/workflow/status/Xtley001/corvus/ci.yml?branch=main)](https://github.com/Xtley001/corvus/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.77+-orange.svg)](https://www.rust-lang.org)
[![Solidity](https://img.shields.io/badge/solidity-0.8.24-363636.svg)](https://soliditylang.org)

Corvus monitors decentralized lending markets across 10 EVM chains to execute atomic, uncollateralized liquidations via flash loans. Transactions dynamically calculate profit hurdles from live gas feeds, compute swap slippage from AMM reserves, and route through private builder channels to eliminate front-running. For the complete protocol specification and formal proofs, see the [whitepaper](./docs/whitepaper.md).

## Installation

Ensure Rust 1.77+ and Foundry are installed:

```bash
git clone https://github.com/Xtley001/corvus.git && cd corvus
cd bot && cargo build --release --locked
cd ../contracts && forge build
```

## Quickstart

1. Configure runtime environment:

```bash
cp bot/.env.example bot/.env
# Populate OWNER_ADDRESS, EXECUTOR_PRIVATE_KEY, and COLD_WALLET_ADDRESS
```

2. Run the engine against your configured node:

```bash
cd bot && cargo run --release --bin corvus
```

For setting up a dedicated high-throughput execution client, see [Node Setup](./docs/NODE_SETUP.md).

## Architecture

```
corvus/
├── bot/
│   ├── config/             # Multi-chain TOML configurations
│   └── src/
│       ├── chains/         # Market adapters & registry (10 chains)
│       ├── flash/          # Balancer, Morpho, Aave, HyperLend flash adapters
│       ├── swap/           # AMM venue adapters (UniV3, Curve, Aerodrome, etc.)
│       ├── shared/         # REVM engine, gas oracle, mempool monitor
│       └── strategies/     # Dynamic liquidation dispatch & sizing
├── contracts/              # Unified ExecutorBase flash engine (Foundry)
├── monitoring/             # Prometheus & Grafana telemetry
├── docs/                   # Extended docs, node setup, and whitepaper
└── scripts/                # Verification and deployment utilities
```

For complete system design, adapter traits, and data flows, see [Architecture](./docs/ARCHITECTURE.md).

## Supported Networks

| Network | Chain ID | Lending Markets | Flash Providers | Submission Mode |
|---|---|---|---|---|
| Ethereum | 1 | Aave V3, Spark, Morpho Blue | Balancer V2, Morpho, Aave V3 | Private (Flashbots Protect) |
| Base | 8453 | Aave V3, Morpho Blue | Balancer V3/V2, Morpho, Aave V3 | Private (MEV-Protected RPC) |
| Arbitrum One | 42161 | Aave V3 | Aave V3 | Private (Timeboost) |
| Polygon PoS | 137 | Aave V3 | Aave V3 | Private (Bor Private Mempool) |
| BNB Chain | 56 | Aave V3 | Aave V3 | Private (bloXroute Relay) |
| HyperEVM | 999 | HyperLend | HyperLend Native | Direct Sequencer |
| Optimism | 10 | Aave V3 | Aave V3 | Direct Sequencer |
| Avalanche C-Chain | 43114 | Aave V3 | Aave V3 | Direct Sequencer |
| Gnosis Chain | 100 | Aave V3 | Aave V3, Balancer V2 | Direct Sequencer |
| Plasma | 9999 | Aave V3 | Aave V3 | Direct Sequencer |

## Testing

Run unit and integration test suites:

```bash
# Test Rust engine & chain adapters
cd bot && cargo test --release --lib

# Run contract tests
cd ../contracts && forge test -vv
```

## Security

All transactions execute through the non-reentrant `ExecutorBase` contract with strict protocol whitelists and atomic solvency checks. Report vulnerabilities per our [security policy](./SECURITY.md).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development environment setup and pull request guidelines.

## License

Released under the [MIT License](./LICENSE).
