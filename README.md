# Goshawk

*Autonomous Ethereum Mainnet protocol liquidation engine with dynamic risk modeling and private builder submission.*

[![CI](https://img.shields.io/github/actions/workflow/status/Xtley001/goshawk/ci.yml?branch=main)](https://github.com/Xtley001/goshawk/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.77+-orange.svg)](https://www.rust-lang.org)
[![Solidity](https://img.shields.io/badge/solidity-0.8.24-363636.svg)](https://soliditylang.org)

Goshawk monitors decentralized lending markets across Ethereum Mainnet (Chain ID 1) to execute atomic, uncollateralized liquidations via flash loans. Transactions dynamically calculate profit hurdles from live gas feeds, compute swap slippage from AMM reserves, enforce dual independent economic gates, and route through private Ethereum builder relays to eliminate front-running. For the complete protocol specification and formal proofs, see the [whitepaper](./docs/whitepaper.md).

## Installation

Ensure Rust 1.77+ and Foundry are installed:

```bash
git clone https://github.com/Xtley001/goshawk.git && cd goshawk
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
cd bot && cargo run --release --bin goshawk
```

For setting up a dedicated high-throughput execution client, see [Node Setup](./docs/NODE_SETUP.md).

## Architecture

```
goshawk/
├── bot/
│   ├── config/             # Ethereum TOML configuration
│   └── src/
│       ├── chains/         # Market adapters & registry (Ethereum Mainnet)
│       ├── flash/          # Morpho Blue, Balancer V2, Spark DSS, Aave V3 flash adapters
│       ├── swap/           # AMM venue adapters (Uniswap V3, Curve, Balancer)
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
| Ethereum Mainnet | 1 | Aave V3, Morpho Blue, Spark, Fluid, Compound V3, Euler V2 | Morpho Blue, Balancer V2, Spark DSS Flash, Aave V3 | Private (Flashbots, Titan, Beaver) |

## Testing

Run unit and integration test suites:

```bash
# Test Rust engine & chain adapters
cd bot && cargo test --release --lib

# Run integration tests
cd bot && cargo test --test integration

# Run contract tests
cd ../contracts && forge test -vv
```

## Security

All transactions execute through the non-reentrant `ExecutorBase` contract with strict protocol whitelists and atomic solvency checks. Report vulnerabilities per our [security policy](./SECURITY.md).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development environment setup and pull request guidelines.

## License

Released under the [MIT License](./LICENSE).
