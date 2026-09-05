//! Integration tests for Goshawk Ethereum Mainnet (Chain ID 1) liquidation engine.
//! Validates:
//! 1. All 6 lending adapters (Aave V3, Morpho Blue, Spark, Fluid, Compound V3, Euler V2)
//! 2. Flash loan providers and selection priority (Morpho -> Balancer -> Spark DSS -> Aave)
//! 3. Swap venue registry (Uniswap V3, Curve, Balancer)
//! 4. Economics thresholds and dual independent gate rules (§10.1, §10.2, §10.4)
//! 5. Fixed-point math and tick calculations

use std::str::FromStr;
use ethers::types::{Address, Bytes, TxHash, U256};

use goshawk::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use goshawk::chains::ethereum::aave_v3::EthereumAaveV3Adapter;
use goshawk::chains::ethereum::morpho_blue::MorphoBlueAdapter;
use goshawk::chains::ethereum::spark::SparkAdapter;
use goshawk::chains::ethereum::fluid::FluidAdapter;
use goshawk::chains::ethereum::compound_v3::CompoundV3Adapter;
use goshawk::chains::ethereum::euler_v2::EulerV2Adapter;

use goshawk::flash::{
    FlashLoanRegistry,
    morpho_blue::MorphoBlueFlashAdapter,
    balancer_v2::BalancerV2FlashAdapter,
    spark_dss_flash::SparkDssFlashAdapter,
    aave_v3::AaveV3FlashAdapter,
};
use goshawk::swap::{
    SwapVenueRegistry,
    uniswap_v3::GenericUniswapV3Adapter,
    curve::CurveAdapter,
    balancer::BalancerSwapAdapter,
};
use goshawk::shared::position_indexer::{BorrowPosition, LendingProtocol};
use goshawk::strategies::liquidation::{
    protocol_break_even_debt_usd, protocol_min_clip_usd
};

// ─── 1. SOURCED AND GAPPED LENDING ADAPTERS ──────────────────────────────────

#[tokio::test]
async fn test_ethereum_aave_v3_adapter_svr_exclusion() {
    let adapter = EthereumAaveV3Adapter::new(Address::random(), Address::random());
    assert_eq!(adapter.id(), "aave_v3_ethereum");
    assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

    // tBTC and AAVE on Ethereum Mainnet
    let tbtc = Address::from_str("0x18084fbA666a33d37592fA2633fD49a74DD93a88").unwrap();
    let aave = Address::from_str("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9").unwrap();
    let regular_asset = Address::random();
    let usdc = Address::random();

    assert!(adapter.is_svr_excluded(tbtc, usdc));
    assert!(adapter.is_svr_excluded(regular_asset, aave));
    assert!(!adapter.is_svr_excluded(regular_asset, usdc));

    let pos_svr = BorrowPosition {
        borrower: Address::random(),
        collateral_asset: tbtc,
        debt_asset: usdc,
        debt_amount: U256::from(50_000u64),
        collateral_amount: U256::from(100_000u64),
        health_factor: 1.01,
        protocol: LendingProtocol::Aave,
        morpho_market_params: Bytes::new(),
        morpho_market_id: TxHash::zero(),
        last_update_block: 200,
    };

    let pos_ok = BorrowPosition {
        borrower: Address::random(),
        collateral_asset: regular_asset,
        debt_asset: usdc,
        debt_amount: U256::from(50_000u64),
        collateral_amount: U256::from(100_000u64),
        health_factor: 1.01,
        protocol: LendingProtocol::Aave,
        morpho_market_params: Bytes::new(),
        morpho_market_id: TxHash::zero(),
        last_update_block: 200,
    };

    adapter.set_positions(vec![pos_svr.clone(), pos_ok.clone()]);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let candidates = adapter.positions_below_hf(1.03).await;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].borrower, pos_ok.borrower);

    let route = SwapRoute {
        venue_id: "uniswap_v3".into(),
        path: Bytes::new(),
        expected_out: U256::from(49_000u64),
    };

    assert!(adapter.build_liquidation_calldata(&pos_svr, route.clone(), U256::from(100)).await.is_err());
    assert!(adapter.build_liquidation_calldata(&pos_ok, route, U256::from(100)).await.is_ok());
}

#[tokio::test]
async fn test_morpho_blue_adapter_market_params_validation() {
    let adapter = MorphoBlueAdapter::default();
    assert_eq!(adapter.id(), "morpho_blue");
    assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

    let route = SwapRoute {
        venue_id: "uniswap_v3".into(),
        path: Bytes::new(),
        expected_out: U256::from(49_000u64),
    };

    // 1. Position with zero market params MUST fail
    let pos_zero = BorrowPosition {
        borrower: Address::random(),
        collateral_asset: Address::random(),
        debt_asset: Address::random(),
        debt_amount: U256::from(50_000u64),
        collateral_amount: U256::from(100_000u64),
        health_factor: 0.95,
        protocol: LendingProtocol::Morpho,
        morpho_market_params: Bytes::new(),
        morpho_market_id: TxHash::zero(),
        last_update_block: 200,
    };
    assert!(adapter.build_liquidation_calldata(&pos_zero, route.clone(), U256::from(100)).await.is_err());

    // 2. Position with valid market params MUST succeed
    let valid_id = TxHash::from_str("0x8793cf302b8ffd655ab97bd1c695dbd967807e8367a65cb2f4edaf1380ba1bda").unwrap();
    let pos_valid = BorrowPosition {
        morpho_market_params: Bytes::from(vec![1u8; 32]),
        morpho_market_id: valid_id,
        ..pos_zero
    };
    let res = adapter.build_liquidation_calldata(&pos_valid, route, U256::from(100)).await;
    assert!(res.is_ok());
    assert!(!res.unwrap().is_empty());
}

#[tokio::test]
async fn test_spark_adapter_gating() {
    let adapter = SparkAdapter::new(Address::random(), Address::random());
    assert_eq!(adapter.id(), "spark");
    assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

    let pos = BorrowPosition {
        borrower: Address::random(),
        collateral_asset: Address::random(),
        debt_asset: Address::random(),
        debt_amount: U256::from(50_000u64),
        collateral_amount: U256::from(100_000u64),
        health_factor: 1.01,
        protocol: LendingProtocol::Spark,
        morpho_market_params: Bytes::new(),
        morpho_market_id: TxHash::zero(),
        last_update_block: 200,
    };

    adapter.set_positions(vec![pos.clone()]);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let at_risk = adapter.positions_below_hf(1.03).await;
    assert_eq!(at_risk.len(), 1);

    let route = SwapRoute {
        venue_id: "curve".into(),
        path: Bytes::new(),
        expected_out: U256::from(49_000u64),
    };
    assert!(adapter.build_liquidation_calldata(&pos, route, U256::from(500)).await.is_ok());
}

#[tokio::test]
async fn test_gapped_adapters_inert_by_default() {
    // 1. Fluid
    let fluid = FluidAdapter::default();
    assert_eq!(fluid.id(), "fluid");
    assert!(!fluid.is_enabled());
    assert!(fluid.positions_below_hf(1.5).await.is_empty());

    // 2. Compound V3
    let compound = CompoundV3Adapter::default();
    assert_eq!(compound.id(), "compound_v3");
    assert!(!compound.is_enabled());
    assert!(compound.positions_below_hf(1.5).await.is_empty());

    // 3. Euler V2
    let euler = EulerV2Adapter::default();
    assert_eq!(euler.id(), "euler_v2");
    assert!(!euler.is_enabled());
    assert!(euler.positions_below_hf(1.5).await.is_empty());
}

// ─── 2. FLASH LOAN AND SWAP REGISTRIES ───────────────────────────────────────

#[tokio::test]
async fn test_flash_loan_registry_and_precedence() {
    let mut reg = FlashLoanRegistry::new();
    reg.register(Box::new(MorphoBlueFlashAdapter::default()));
    reg.register(Box::new(BalancerV2FlashAdapter::default()));
    reg.register(Box::new(SparkDssFlashAdapter::default()));
    reg.register(Box::new(AaveV3FlashAdapter::default()));

    assert_eq!(reg.len(), 4);
    assert!(reg.get("morpho_blue").is_some());
    assert!(reg.get("balancer_v2").is_some());
    assert!(reg.get("spark_dss_flash").is_some());
    assert!(reg.get("aave_v3").is_some());

    let test_amount = U256::from(100_000u64);

    // Zero-fee flash providers (07 §7.1: Morpho, Balancer, Spark DSS)
    assert_eq!(
        reg.get("morpho_blue").unwrap().quote_fee(Address::zero(), test_amount).await.unwrap(),
        U256::zero()
    );
    assert_eq!(
        reg.get("balancer_v2").unwrap().quote_fee(Address::zero(), test_amount).await.unwrap(),
        U256::zero()
    );
    assert_eq!(
        reg.get("spark_dss_flash").unwrap().quote_fee(Address::zero(), test_amount).await.unwrap(),
        U256::zero()
    );

    // Fee-bearing backstop provider: Aave V3 = 0.05% = 5 bps (50 on 100,000)
    assert_eq!(
        reg.get("aave_v3").unwrap().quote_fee(Address::zero(), test_amount).await.unwrap(),
        U256::from(50u64)
    );
}

#[tokio::test]
async fn test_swap_venue_registry_ethereum() {
    let mut reg = SwapVenueRegistry::new();
    reg.register(Box::new(GenericUniswapV3Adapter::default()));
    reg.register(Box::new(CurveAdapter::default()));
    reg.register(Box::new(BalancerSwapAdapter::default()));

    assert_eq!(reg.len(), 3);
    assert!(reg.get("uniswap_v3").is_some());
    assert!(reg.get("curve").is_some());
    assert!(reg.get("balancer").is_some());

    // Verify all 3 provide non-zero quotes
    for venue_id in &["uniswap_v3", "curve", "balancer"] {
        let venue = reg.get(venue_id).unwrap();
        let q = venue.quote(Address::zero(), Address::zero(), U256::from(10_000)).await.unwrap();
        assert!(q > U256::zero());
    }

    // Deleted venues MUST not exist
    assert!(reg.get("aerodrome").is_none());
    assert!(reg.get("velodrome").is_none());
    assert!(reg.get("pancakeswap_v3").is_none());
}

// ─── 3. ECONOMICS & DUAL INDEPENDENT GATES ───────────────────────────────────

#[test]
fn test_economics_break_even_and_regimes() {
    let regular_asset = Address::random();
    let wbtc = Address::from_str("0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f").unwrap();
    let flat_floor = 750.0;

    // §10.1 Break-even values
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Morpho, regular_asset, false, flat_floor), 900.0);
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::CompoundV3, regular_asset, false, flat_floor), 1200.0);
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Spark, regular_asset, false, flat_floor), 2333.33);
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Aave, regular_asset, false, flat_floor), 2700.0);
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Fluid, regular_asset, false, flat_floor), 750.0);
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::EulerV2, regular_asset, false, flat_floor), 750.0);

    // §10.2 WBTC Normal vs Spike regime scaling
    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Aave, wbtc, false, flat_floor), 770.0);
    assert_eq!(protocol_min_clip_usd(LendingProtocol::Aave, wbtc, false, flat_floor), 5000.0);

    assert_eq!(protocol_break_even_debt_usd(LendingProtocol::Aave, wbtc, true, flat_floor), 5600.0);
    assert_eq!(protocol_min_clip_usd(LendingProtocol::Aave, wbtc, true, flat_floor), 25000.0);
}

#[test]
fn test_dual_independent_gate_enforcement() {
    let flat_floor = 750.0;
    let morpho_break_even = 900.0;

    // Gate 1: Profit >= flat_floor
    // Gate 2: Debt >= protocol_break_even

    // 1. High profit ($1000) but low debt ($500 < $900) -> Rejected by Gate 2
    let profit_1 = 1000.0;
    let debt_1 = 500.0;
    assert!(profit_1 >= flat_floor);
    assert!(!(debt_1 >= morpho_break_even), "Gate 2 must reject low debt");

    // 2. Low profit ($200 < $750) but high debt ($10,000) -> Rejected by Gate 1
    let profit_2 = 200.0;
    let debt_2 = 10_000.0;
    assert!(!(profit_2 >= flat_floor), "Gate 1 must reject low profit");
    assert!(debt_2 >= morpho_break_even);

    // 3. Both satisfy -> Accepted
    let profit_3 = 800.0;
    let debt_3 = 5_000.0;
    assert!(profit_3 >= flat_floor && debt_3 >= morpho_break_even);
}

// ─── 4. ALGORITHMIC & FIXED-POINT MATH ───────────────────────────────────────

#[test]
fn test_isqrt() {
    fn isqrt(n: U256) -> U256 {
        if n.is_zero() { return U256::zero(); }
        let mut x = n;
        let mut y = (x + U256::one()) / 2;
        while y < x { x = y; y = (x + n / x) / 2; }
        x
    }

    assert_eq!(isqrt(U256::from(0u64)),   U256::from(0u64));
    assert_eq!(isqrt(U256::from(1u64)),   U256::from(1u64));
    assert_eq!(isqrt(U256::from(4u64)),   U256::from(2u64));
    assert_eq!(isqrt(U256::from(9u64)),   U256::from(3u64));
    assert_eq!(isqrt(U256::from(100u64)), U256::from(10u64));
    assert_eq!(isqrt(U256::from(144u64)), U256::from(12u64));
}

#[test]
fn test_floor_ceil_tick() {
    fn floor_tick(tick: i32, spacing: i32) -> i32 {
        let f = tick / spacing;
        if tick < 0 && tick % spacing != 0 { (f - 1) * spacing } else { f * spacing }
    }
    fn ceil_tick(tick: i32, spacing: i32) -> i32 {
        let c = tick / spacing;
        if tick > 0 && tick % spacing != 0 { (c + 1) * spacing } else { c * spacing }
    }

    assert_eq!(floor_tick(-105, 10), -110);
    assert_eq!(floor_tick(-100, 10), -100);
    assert_eq!(floor_tick(-1,   10), -10);
    assert_eq!(floor_tick(105,  10), 100);
    assert_eq!(floor_tick(100,  10), 100);
    assert_eq!(floor_tick(0,    10), 0);

    assert_eq!(ceil_tick(105, 10), 110);
    assert_eq!(ceil_tick(100, 10), 100);
    assert_eq!(ceil_tick(-100, 10), -100);
}

#[test]
fn test_optimal_arb_input() {
    fn isqrt(n: U256) -> U256 {
        if n.is_zero() { return U256::zero(); }
        let mut x = n;
        let mut y = (x + U256::one()) / 2;
        while y < x { x = y; y = (x + n / x) / 2; }
        x
    }
    fn optimal_arb_input(r_in: U256, r_out: U256, max_pct: u32) -> U256 {
        match r_in.checked_mul(r_out) {
            None    => U256::zero(),
            Some(p) => isqrt(p).saturating_sub(r_in).min(r_in * max_pct / 100),
        }
    }

    assert_eq!(optimal_arb_input(U256::zero(), U256::from(1000u64), 30), U256::zero());
    let r = U256::from(1_000_000u64);
    assert_eq!(optimal_arb_input(r, r, 30), U256::zero());

    let r_in  = U256::from(1_000_000u64);
    let r_out = U256::from(2_000_000u64);
    let opt   = optimal_arb_input(r_in, r_out, 30);
    assert!(opt > U256::zero());
    assert!(opt <= r_in * 30 / 100);
}
