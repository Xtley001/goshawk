# Architecture

*Deep technical specification of the Goshawk Ethereum Mainnet liquidation engine, execution pipeline, and risk controls.*

Goshawk is a specialized protocol liquidation system operating exclusively on Ethereum Mainnet (Chain ID 1). It enforces zero capital-at-risk execution by borrowing debt capital via uncollateralized flash loans, liquidating underwater borrow positions across 6 supported lending protocols, swapping seized collateral back to the borrowed asset, and repaying the flash loan in a single atomic transaction.

```mermaid
flowchart TD
    BlockStream[Confirmed Block Stream / IPC] --> Indexer[Position Indexer & State Cache]
    Indexer --> PriceFeed[Live Chainlink Oracle Feeds]
    PriceFeed --> HfEngine[Dynamic HF Gate & Volatility Filter]
    HfEngine --> Candidates[Eligible Liquidations]
    Candidates --> REVM[Local REVM Fork Simulation]
    Candidates --> Gate2[Gate 2: Protocol Break-even & Min Clip]
    Gate2 --> REVM
    REVM --> Sizing[Realized Slippage & Optimal Sizing]
    Sizing --> FlashRouter[Parallel Live Flash Quoting]
    FlashRouter --> Gate1[Gate 1: Dynamic Gas Floor & Flat Floor]
    Gate1 --> Signer[Local EIP-1559 Signer]
    Signer --> PrivateRelay[Private Builder Auction Relays]
    PrivateRelay --> OnChain[FlashExecutor Contract]
    OnChain --> Sweep[Cold Wallet Sweep]
```

## System Topology

```
goshawk/
├── bot/
│   ├── config/             # Ethereum TOML configuration
│   └── src/
│       ├── chains/         # Lending market adapters & registry
│       │   └── ethereum/   # Aave V3, Morpho Blue, Spark, Fluid, Compound V3, Euler V2
│       ├── flash/          # Morpho Blue, Balancer V2, Spark DSS, Aave V3 flash adapters
│       ├── swap/           # DEX adapters (Uniswap V3, Curve, Balancer)
│       ├── shared/         # REVM engine, gas oracle, mempool monitor
│       └── strategies/     # Dynamic liquidation dispatch & dual-gate sizing
├── contracts/
│   └── src/
│       ├── ExecutorBase.sol # Unified flash execution engine with Chain ID 1 guard
│       ├── FlashExecutor.sol# Deployed executor implementation
│       └── interfaces/     # Protocol interfaces (Aave, Morpho, Balancer, Spark, etc.)
├── monitoring/             # Prometheus metrics and Grafana dashboards
├── scripts/                # Verification, deployment, and sweep utilities
└── docs/                   # Extended documentation, node setup, and formal whitepaper
```

## Trait Architecture

Goshawk decouples protocol mechanics from chain execution using three core traits:

### 1. LendingMarketAdapter

Defines position filtering, health factor evaluation, and calldata construction:

```rust
#[async_trait]
pub trait LendingMarketAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn oracle_kind(&self) -> OracleKind;
    async fn refresh_positions(&self) -> Result<()>;
    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition>;
    async fn build_liquidation_calldata(
        &self,
        pos: &BorrowPosition,
        route: SwapRoute,
        min_profit: U256,
    ) -> Result<Bytes>;
}
```

- **Safety Gate:** Registration explicitly rejects any market reporting `OracleKind::SpotAmm` at boot to prevent atomic price manipulation exploits. All in-scope protocols use `OracleKind::Chainlink`.
- **SVR Exclusion:** Aave V3 adapter enforces strict filtering excluding Soft-Volatile Reserves (tBTC, AAVE) where liquidation spreads are constrained or volatile.

### 2. FlashLoanAdapter

Standardizes parallel depth and fee quoting across flash providers:

```rust
#[async_trait]
pub trait FlashLoanAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    async fn quote_fee(&self, token: Address, amount: U256) -> Result<U256>;
    fn build_flash_call(&self, token: Address, amount: U256, inner_calldata: Bytes) -> Bytes;
}
```

Flash providers are attempted in strict fee order per 07 §7.2:
1. `morpho_blue` (0.00%)
2. `balancer_v2` (0.00%)
3. `spark_dss_flash` (0.00%)
4. `aave_v3` (0.05% fee backstop)

### 3. SwapVenueAdapter

Provides venue-agnostic quote derivation and swap calldata generation:

```rust
#[async_trait]
pub trait SwapVenueAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    async fn quote(&self, token_in: Address, token_out: Address, amount_in: U256) -> Result<U256>;
    fn build_swap_call(&self, token_in: Address, token_out: Address, amount_in: U256, min_out: U256) -> Bytes;
}
```

Supported venues on Ethereum Mainnet: `uniswap_v3`, `curve`, `balancer`.

## Dynamic Threshold & Dual Independent Gates Engine

All operational thresholds are computed continuously per block:

### 1. Dual Independent Gates (§10.4)
Goshawk enforces two separate, non-waivable gates before dispatching an on-chain transaction:
- **Gate 1 (Profit Gate):** Projected net profit must satisfy:
  $$\text{Profit} \ge \max(C_{\text{gas}} \cdot \gamma + C_{\text{opp}}, \text{Floor}_{\text{flat}})$$
- **Gate 2 (Debt & Clip Gate):** Position debt must satisfy:
  $$\text{Debt} \ge \text{BreakEvenDebt}_{\text{protocol}} \quad \text{and} \quad \text{Debt} \ge \text{MinClip}_{\text{protocol}}$$

Clearing one gate never waives the other.

### 2. Break-Even Sizing & Gas Regimes (§10.1–10.2)
Break-even debt thresholds account for Ethereum L1 base fees and priority tips:
- **Morpho Blue:** $900.00
- **Compound V3:** $1,200.00
- **Spark:** $2,333.33
- **Aave V3:** $2,700.00 (Normal: $770 debt / $5,000 clip; Spike >60 gwei: $5,600 debt / $25,000 clip for WBTC)
- **Fluid & Euler V2:** Fallback to flat floor ($750.00) pending market maturity

### 3. Volatility-Widened Health Factor Gate
During rapid market movements, liquidation competition intensifies. Goshawk monitors rolling standard deviations of oracle updates ($\sigma$) and widens the search gate:

$$HF_{\text{gate}} = HF_{\text{base}} + \min(\sigma \cdot \alpha, \Delta_{\max})$$

This permits early pre-simulation and routing before positions cross into insolvency.

### 4. Adaptive Circuit Breaker
Reverts on Ethereum Mainnet cost hundreds of dollars in burned gas. The adaptive breaker weights revert penalties by realized gas loss:

$$P = \min\left(5, 1 + \frac{\text{Loss}_{\text{USD}}}{5}\right)$$

Breakers trip when the accumulated loss or weighted penalty exceeds threshold limits, resetting after 3 consecutive successful transactions.

## Smart Contract Architecture

The system executes through `FlashExecutor.sol`, inheriting from `ExecutorBase.sol`:

### Flash Callback Branches
`ExecutorBase` routes five flash loan callback interfaces into a single shared internal execution routine:

1. **Morpho Blue:** `onMorphoFlashLoan(uint256, bytes)`
2. **Balancer V2:** `receiveFlashLoan(IERC20[], uint256[], uint256[], bytes)`
3. **Spark DSS Flash (ERC-3156):** `onFlashLoan(address, address, uint256, uint256, bytes)`
4. **Aave V3:** `executeOperation(address, uint256, uint256, address, bytes)`
5. **Uniswap V3:** `uniswapV3FlashCallback(uint256, uint256, bytes)`

### Core Invariants
- **Chain ID Guard:** Reverts if deployed on or called from any chain other than Ethereum Mainnet (`block.chainid == 1`).
- **Non-Reentrant:** All execution paths are locked against reentrancy.
- **Whitelist Enforcement:** Interactions are restricted to pre-approved lending pools and flash vaults.
- **Zero Standing Allowances:** Router token approvals are explicitly cleared to zero following every swap.
- **Solvency Gate:** Reverts unconditionally if post-swap balances cannot cover flash debt plus accrued premiums.
- **Dedicated Sweep Paths:** Owner sweeps to arbitrary destination via `sweep()`, executor hot wallet emergency sweeps exclusively to immutable `coldWallet` via `emergencySweep()`.

## Private Submission Architecture

Transactions bypass public mempools on Ethereum Mainnet to eliminate searcher front-running, sandwich attacks, and information leakage:

| Relay | Endpoint | Failure Handling |
|---|---|---|
| Flashbots Relay | `https://relay.flashbots.net` | Active primary |
| Titan Builder | `https://rpc.titanbuilder.xyz` | Failover after 3 consecutive errors |
| Beaver Build | `https://rpc.beaverbuild.com` | Failover after 3 consecutive errors |
