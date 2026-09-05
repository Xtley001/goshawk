# goshawk

Autonomous Ethereum Mainnet liquidation engine with dynamic risk modeling and private builder submission.

[![CI](https://img.shields.io/github/actions/workflow/status/Xtley001/goshawk/ci.yml?branch=master)](https://github.com/Xtley001/goshawk/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust: 1.77+](https://img.shields.io/badge/rust-1.77+-orange.svg)](https://www.rust-lang.org)
[![Solidity: 0.8.24](https://img.shields.io/badge/solidity-0.8.24-363636.svg)](https://soliditylang.org)

Goshawk monitors decentralized lending protocols on Ethereum Mainnet (Chain ID 1) to execute atomic, uncollateralized liquidations via flash loans. The engine dynamically computes gas hurdles and AMM slippage per block, enforces dual independent economic gates, and routes transactions exclusively through private builder auctions to eliminate front-running. For the formal protocol specification and mathematical proofs, see the [whitepaper](./docs/whitepaper.md).

## Installation

Ensure Rust 1.77+ and Foundry are installed:

```bash
git clone https://github.com/Xtley001/goshawk.git && cd goshawk
cd bot && cargo build --release --locked
cd ../contracts && forge build
```

## Quickstart

Configure environment variables and start the engine:

```bash
cp bot/.env.example bot/.env
# Configure GOSHAWK_HOT_PRIVATE_KEY, GOSHAWK_COLD_WALLET, and ETH_RPC_URL

cd bot && cargo run --release --bin goshawk
```

For provisioning a dedicated local execution client with sub-millisecond IPC latency, see [Node Setup](./docs/NODE_SETUP.md).

## Architecture

```
goshawk/
├── bot/
│   ├── config/             # Ethereum TOML runtime configuration
│   └── src/
│       ├── chains/         # Lending market adapters (Aave, Morpho, Spark, Fluid, Compound, Euler)
│       ├── flash/          # Flash loan adapters (Morpho, Balancer, Spark DSS, Aave)
│       ├── swap/           # DEX execution venues (Uniswap V3, Curve, Balancer)
│       ├── shared/         # REVM simulation, gas oracle, mempool monitor
│       └── strategies/     # Liquidation dispatch and dual-gate sizing
├── contracts/              # FlashExecutor and ExecutorBase contracts (Foundry)
├── monitoring/             # Prometheus metrics and Grafana dashboards
├── scripts/                # Deployment, verification, and sweep scripts
└── docs/                   # System architecture, node setup, and whitepaper
```

For detailed system topology, adapter traits, and invariant specifications, see [Architecture](./docs/ARCHITECTURE.md).

## Protocols and Deployed Contracts

| Protocol | Type | Contract Address | Status |
|---|---|---|---|
| Aave V3 | Lending Pool Proxy | `0x794a61358D6845594F94dc1DB02A252b5b4814aD` | Active |
| Morpho Blue | Lending & Flash Core | `0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb` | Active |
| Balancer V2 | Flash Vault | `0xBA12222222228d8Ba53be47888D16304ca09907c` | Active |
| Spark DSS Flash | Flash Mint (ERC-3156) | `0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA` | Active |
| Spark SparkLend | Lending Pool Proxy | `Unverified Gap` | Disabled (Pending Verification) |
| Fluid | Liquidity Layer | `Unverified Gap` | Disabled (Pending Verification) |
| Compound V3 | Comet Proxy | `Unverified Gap` | Disabled (Pending Verification) |
| Euler V2 | Vault Controller | `Unverified Gap` | Disabled (Pending Verification) |

Verify all addresses and oracle aggregators against a live RPC node using:

```bash
ETH_RPC_URL=http://127.0.0.1:8545 ./scripts/verify_addresses.sh
```

## Testing

Run unit and integration test suites:

```bash
# Run Rust unit tests
cd bot && cargo test --release --lib

# Run integration tests (adapters, registries, dual gates)
cd bot && cargo test --test integration

# Run Foundry contract tests
cd ../contracts && forge test -vv
```

## Security

All liquidation calls execute through `FlashExecutor.sol`, enforcing strict protocol whitelists, caller authorization, and atomic solvency checks. Net realized profits sweep directly to an immutable cold wallet. Report vulnerabilities per our [security policy](./SECURITY.md).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for environment setup and pull request guidelines.

## License

Released under the [MIT License](./LICENSE).
