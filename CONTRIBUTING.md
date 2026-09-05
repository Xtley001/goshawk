# Contributing

*Guidelines for engineering contributions to the Goshawk protocol.*

Goshawk executes financial transactions with real capital via automated systems. Code quality, correctness, and review discipline take absolute priority over deployment speed.

## Development Setup

### Prerequisites

- **Rust:** Stable 1.77+ (`rustup update stable`)
- **Foundry:** Latest release (`foundryup`)
- **Node.js:** 18+ (for contract linting and tooling)

### Building the Project

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
| Build | `cargo build --release --locked` | Rust core engine |
| Lint | `cargo clippy --release --locked -- -D warnings` | Clippy zero-warning enforcement |
| Unit Tests | `cargo test --release --lib` | Ethereum adapters, math, flash routers |
| Contract Build | `forge build` | `ExecutorBase.sol` and interfaces |
| Contract Tests | `forge test -vv` | Unit and flash callback tests |

## Contribution Guidelines

- **No Public Vulnerabilities:** Never report security bugs in public issues or PRs. Follow [SECURITY.md](./SECURITY.md).
- **Zero Spot AMM Oracles:** Never register an adapter utilizing `OracleKind::SpotAmm`. Liquidation eligibility must never rely on same-block manipulable prices.
- **Contract Changes:** Any modification to `ExecutorBase.sol` requires a corresponding Foundry test asserting that post-trade solvency is preserved and profits sweep to `coldWallet`.
- **Commit Messages:** Follow the Conventional Commits specification (e.g. `feat(chains): add Plasma adapter`, `fix(gas): correct 75th percentile tip indexing`).

## Pull Request Process

1. Fork the repository and create a branch from `master`.
2. Ensure all quality gates pass locally.
3. Submit a pull request detailing the problem, technical implementation, and verification steps performed.
4. Obtain review approval from at least one core maintainer before merge.
