# Contributing

Guidelines for engineering contributions to the Goshawk engine.

Goshawk executes uncollateralized financial transactions with live capital on Ethereum Mainnet. Code quality, formal correctness, and deterministic test coverage take priority over deployment speed.

## Development Setup

### Prerequisites

- **Rust:** Stable 1.77+ (`rustup update stable`)
- **Foundry:** Latest release (`foundryup`)
- **Bash:** 4.0+ (for verification and operational scripts)

### Building

```bash
# Build Rust engine
cd bot && cargo build --release --locked

# Build smart contracts
cd ../contracts && forge build
```

## Quality Gates

All pull requests must pass the automated CI pipeline before merge:

| Gate | Command | Scope |
|---|---|---|
| Engine Build | `cargo build --release --locked` | Rust core engine |
| Engine Unit Tests | `cargo test --release --lib` | Adapters, economics, math |
| Integration Tests | `cargo test --test integration` | Full adapter & registry validation |
| Contract Build | `forge build` | `FlashExecutor.sol` and interfaces |
| Contract Tests | `forge test -vv` | Callback invariants and solvency tests |

## Contribution Guidelines

- **Zero Spot AMM Oracles:** Never register an adapter utilizing `OracleKind::SpotAmm`. All liquidation evaluation must consume tamper-resistant Chainlink feeds.
- **Zero Address Fabrication:** Never invent, guess, or copy unverified contract addresses. Any unsourced contract must remain an explicit gap until confirmed against live on-chain bytecode via `scripts/verify_addresses.sh`.
- **Dual Independent Gates:** Any adjustment to liquidation evaluation must preserve the independence of Gate 1 (profit hurdle) and Gate 2 (debt hurdle). Clearing one gate must never waive the other.
- **Contract Solvency:** Any modification to `ExecutorBase.sol` or `FlashExecutor.sol` requires a corresponding Foundry test asserting that post-trade solvency is preserved and profits sweep to `coldWallet`.
- **Commit Messages:** Follow the Conventional Commits specification (e.g. `feat(chains): add verified Spark pool proxy`, `fix(gas): correct priority fee multiplier`).

## Pull Request Process

1. Fork the repository and create a feature branch from `master`.
2. Ensure all quality gates pass locally.
3. Verify that `scripts/verify_addresses.sh` passes against an Ethereum RPC if addresses or oracles were modified.
4. Submit a pull request detailing the changes, risk assessment, and verification output.
5. Obtain approval from at least one core maintainer before merge.
