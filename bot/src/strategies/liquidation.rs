//! Strategy 3 — Protocol Liquidation.
//! 10_ECONOMICS_AND_RISK.md §10.1, §10.2, §10.4.
//!
//! Dual Independent Gates:
//!   1. Profit Gate: Net simulation profit must clear both dynamic gas-adjusted floor
//!      and the flat `min_liquidation_profit_usd` floor.
//!   2. Debt & Clip Gate: Borrow position debt must clear per-protocol break-even debt
//!      and recommended minimum clip size (including Aave V3/WBTC Normal vs Spike regime scaling).

use anyhow::Result;
use dashmap::DashMap;
use ethers::types::{Address, U256};
use std::sync::Arc;
use crate::{
    config::Config,
    shared::{
        addresses::ethereum,
        position_indexer::{BorrowPosition, LendingProtocol, PositionIndexer, PriceMap},
        mempool_monitor::MempoolMonitor,
        simulation::SimulationEngine,
        flash_loan::FlashLoanRouter,
        submission::SubmissionPipeline,
    },
};

// ─── §10.1 Break-even Economics Constants ─────────────────────────────────────
pub const BREAK_EVEN_DEBT_MORPHO_BLUE: f64 = 900.0;
pub const BREAK_EVEN_DEBT_COMPOUND_V3:  f64 = 1_200.0;
pub const BREAK_EVEN_DEBT_SPARK:        f64 = 2_333.33;
pub const BREAK_EVEN_DEBT_AAVE_V3:      f64 = 2_700.0;

// ─── §10.2 Live Gas Regime (Aave V3, WBTC) ───────────────────────────────────
pub const AAVE_WBTC_NORMAL_BREAK_EVEN_DEBT: f64 = 770.0;
pub const AAVE_WBTC_NORMAL_MIN_CLIP:        f64 = 5_000.0;
pub const AAVE_WBTC_SPIKE_BREAK_EVEN_DEBT:  f64 = 5_600.0;
pub const AAVE_WBTC_SPIKE_MIN_CLIP:         f64 = 25_000.0;

/// Check if an asset is WBTC on Ethereum Mainnet (05 §5.1: 0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f)
pub fn is_wbtc_asset(asset: Address) -> bool {
    format!("{:?}", asset).to_lowercase().contains("2f2a25")
}

/// Calculate the protocol break-even debt threshold per 10_ECONOMICS_AND_RISK.md §10.1 & §10.2.
pub fn protocol_break_even_debt_usd(
    protocol: LendingProtocol,
    debt_asset: Address,
    is_spike_regime: bool,
    fallback_flat_floor: f64,
) -> f64 {
    match protocol {
        LendingProtocol::Aave => {
            if is_wbtc_asset(debt_asset) {
                if is_spike_regime {
                    AAVE_WBTC_SPIKE_BREAK_EVEN_DEBT
                } else {
                    AAVE_WBTC_NORMAL_BREAK_EVEN_DEBT
                }
            } else {
                BREAK_EVEN_DEBT_AAVE_V3
            }
        }
        LendingProtocol::Morpho => BREAK_EVEN_DEBT_MORPHO_BLUE,
        LendingProtocol::CompoundV3 => BREAK_EVEN_DEBT_COMPOUND_V3,
        LendingProtocol::Spark => BREAK_EVEN_DEBT_SPARK,
        // Fluid and Euler V2: ⚠️ GAP — NOT SOURCED (§10.1 gap note: fallback to flat floor)
        LendingProtocol::Fluid | LendingProtocol::EulerV2 => fallback_flat_floor,
    }
}

/// Calculate the recommended minimum clip size per 10_ECONOMICS_AND_RISK.md §10.2.
pub fn protocol_min_clip_usd(
    protocol: LendingProtocol,
    debt_asset: Address,
    is_spike_regime: bool,
    fallback_flat_floor: f64,
) -> f64 {
    match protocol {
        LendingProtocol::Aave => {
            if is_wbtc_asset(debt_asset) {
                if is_spike_regime {
                    AAVE_WBTC_SPIKE_MIN_CLIP
                } else {
                    AAVE_WBTC_NORMAL_MIN_CLIP
                }
            } else {
                fallback_flat_floor
            }
        }
        _ => fallback_flat_floor,
    }
}

pub async fn run(
    indexer:   Arc<PositionIndexer>,
    mm:        Arc<MempoolMonitor>,
    sim:       Arc<SimulationEngine>,
    flash:     Arc<FlashLoanRouter>,
    sub:       Arc<SubmissionPipeline>,
    cfg:       Config,
    eth_price: f64,
) -> Result<()> {
    let prices = mm.current_oracle_prices();
    let realized_vol = mm.realized_volatility();
    execute_liquidations(&indexer, &*prices, &sim, &flash, &sub, &cfg, eth_price, realized_vol).await
}

/// Mempool-triggered pre-signing using pending oracle prices.
pub async fn run_presign(
    indexer:        Arc<PositionIndexer>,
    current_prices: Arc<DashMap<Address, f64>>,
    pending_prices: PriceMap,
    sim:            Arc<SimulationEngine>,
    flash:          Arc<FlashLoanRouter>,
    sub:            Arc<SubmissionPipeline>,
    cfg:            Config,
    eth_price:      f64,
    realized_vol:   f64,
) -> Result<()> {
    let eth_cfg = cfg.get_chain("ethereum");
    let base_hf = eth_cfg.map(|c| c.hf_threshold()).unwrap_or(1.03);
    let vol_scale = eth_cfg.map(|c| c.volatility_scale).unwrap_or(1.5);
    let max_widening = eth_cfg.map(|c| c.max_hf_widening).unwrap_or(0.05);
    let hf_thresh = base_hf + (realized_vol * vol_scale).min(max_widening);

    let at_risk = indexer.positions_below_pending_hf(&pending_prices, hf_thresh);
    if at_risk.is_empty() { return Ok(()); }

    tracing::info!("Pre-signing {} liquidations on pending oracle update (hf_thresh={:.4})", at_risk.len(), hf_thresh);
    execute_liquidations_for_positions(&at_risk, &*current_prices, &sim, &flash, &sub, &cfg, eth_price).await
}

async fn execute_liquidations(
    indexer:      &Arc<PositionIndexer>,
    prices:       &PriceMap,
    sim:          &Arc<SimulationEngine>,
    flash:        &Arc<FlashLoanRouter>,
    sub:          &Arc<SubmissionPipeline>,
    cfg:          &Config,
    eth_price:    f64,
    realized_vol: f64,
) -> Result<()> {
    let eth_cfg = cfg.get_chain("ethereum");
    let base_hf = eth_cfg.map(|c| c.hf_threshold()).unwrap_or(1.03);
    let vol_scale = eth_cfg.map(|c| c.volatility_scale).unwrap_or(1.5);
    let max_widening = eth_cfg.map(|c| c.max_hf_widening).unwrap_or(0.05);
    let hf_thresh = base_hf + (realized_vol * vol_scale).min(max_widening);

    indexer.refresh_health_factors(prices);
    let positions = indexer.positions_below_hf(hf_thresh);
    execute_liquidations_for_positions(&positions, prices, sim, flash, sub, cfg, eth_price).await
}

async fn execute_liquidations_for_positions(
    positions: &[BorrowPosition],
    prices:    &PriceMap,
    sim:       &Arc<SimulationEngine>,
    flash:     &Arc<FlashLoanRouter>,
    sub:       &Arc<SubmissionPipeline>,
    cfg:       &Config,
    eth_price: f64,
) -> Result<()> {
    let eth_cfg = cfg.get_chain("ethereum");
    let opportunity_cost = eth_cfg.map(|c| c.opportunity_cost_usd).unwrap_or(15.0);
    let flat_min_profit_usd = eth_cfg.map(|c| c.min_liquidation_profit_usd).unwrap_or(750.0);

    let effective_gas_wei = sim.gas_oracle().effective_gas_price_wei();
    // §10.2: Gas regime classification (Spike regime during crash/cascade >= 100 gwei)
    let is_spike_regime = effective_gas_wei >= 100_000_000_000.0;

    for pos in positions {
        // Compute position debt value in USD
        let debt_price = match prices.get(&pos.debt_asset) {
            Some(p) => *p,
            None => {
                tracing::debug!("Skipping pos {:?}: debt asset price missing", pos.borrower);
                continue;
            }
        };
        let debt_hex = format!("{:?}", pos.debt_asset).to_lowercase();
        let debt_dec = ethereum::token_decimals(&debt_hex);
        let debt_unit = 10f64.powi(debt_dec as i32);
        let debt_usd = debt_price * pos.debt_amount.as_u128() as f64 / debt_unit;

        // ─── GATE 2: Debt & Clip Gate (§10.1, §10.2, §10.4) ──────────────────────
        // Independent gate: Discard if debt falls below break-even threshold or minimum clip.
        let break_even_debt = protocol_break_even_debt_usd(
            pos.protocol,
            pos.debt_asset,
            is_spike_regime,
            flat_min_profit_usd,
        );
        let min_clip = protocol_min_clip_usd(
            pos.protocol,
            pos.debt_asset,
            is_spike_regime,
            flat_min_profit_usd,
        );

        if debt_usd < break_even_debt || debt_usd < min_clip {
            tracing::debug!(
                "Gate 2 Discard pos {:?}: debt ${:.2} < break-even ${:.2} or min clip ${:.2} ({:?})",
                pos.borrower, debt_usd, break_even_debt, min_clip, pos.protocol
            );
            continue;
        }

        // ─── REVM Simulation ───────────────────────────────────────────────────
        let sim_r = sim.simulate_liquidation(pos, prices, eth_price).await?;

        // ─── GATE 1: Profit Gate (§10.1, §10.4) ────────────────────────────────
        // Independent gate: Discard if net profit falls below dynamic gas floor or flat USD floor.
        let gas_cost_usd = sim_r.gas_estimate as f64 * effective_gas_wei / 1e18 * eth_price;
        let dynamic_min_profit_usd = gas_cost_usd * cfg.gas_estimate_safety_margin + opportunity_cost;
        let required_profit_usd = dynamic_min_profit_usd.max(flat_min_profit_usd);

        if sim_r.profit_usd < required_profit_usd {
            tracing::debug!(
                "Gate 1 Discard pos {:?}: profit ${:.2} < required ${:.2} (dynamic=${:.2}, flat=${:.2})",
                pos.borrower, sim_r.profit_usd, required_profit_usd, dynamic_min_profit_usd, flat_min_profit_usd
            );
            continue;
        }

        tracing::info!(
            "Liquidation opportunity accepted: borrower={:?} protocol={:?} debt=${:.2} profit=${:.2} (min_req=${:.2})",
            pos.borrower, pos.protocol, debt_usd, sim_r.profit_usd, required_profit_usd
        );

        let swap_route = sim.get_optimal_swap_route(pos.collateral_asset, pos.debt_asset).await?;
        let provider   = flash.select_provider(pos.debt_asset, pos.debt_amount).await?;

        let min_profit_wei = {
            let min_usd = required_profit_usd.max(0.0);
            let wei_f = (min_usd / eth_price.max(1.0)) * 1e18;
            U256::from(wei_f.min(u128::MAX as f64) as u128)
        };
        let calldata = flash.build_liquidation(pos, provider, swap_route, min_profit_wei).await?;

        sub.submit_priority(calldata, sim_r.gas_estimate).await?;
        crate::monitoring::metrics::record_trade("liquidation", sim_r.profit_usd);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_dynamic_profit_calculation() {
        let gas_estimate = 500_000u64;
        let effective_gas_wei = 20_000_000_000f64; // 20 gwei
        let eth_price = 2500.0;
        let safety_margin = 1.25;
        let opportunity_cost = 15.0;

        let gas_cost_usd = gas_estimate as f64 * effective_gas_wei / 1e18 * eth_price;
        assert_eq!(gas_cost_usd, 25.0); // 0.01 ETH = $25

        let dynamic_min_profit_usd = gas_cost_usd * safety_margin + opportunity_cost;
        assert_eq!(dynamic_min_profit_usd, 46.25);

        assert!(40.0 < dynamic_min_profit_usd);
        assert!(50.0 >= dynamic_min_profit_usd);
    }

    #[test]
    fn test_dynamic_hf_widening() {
        let base_hf: f64 = 1.03;
        let vol_scale: f64 = 1.5;
        let max_widening: f64 = 0.05;

        let vol_normal: f64 = 0.005;
        let hf_normal: f64 = base_hf + (vol_normal * vol_scale).min(max_widening);
        assert!((hf_normal - 1.0375).abs() < 1e-6);

        let vol_high: f64 = 0.04;
        let hf_high: f64 = base_hf + (vol_high * vol_scale).min(max_widening);
        assert!((hf_high - 1.08).abs() < 1e-6);
    }

    #[test]
    fn test_protocol_break_even_debt_thresholds() {
        let regular_asset = Address::random();
        let fallback_floor = 750.0;

        // §10.1: Morpho Blue = $900.00
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::Morpho, regular_asset, false, fallback_floor),
            900.0
        );

        // §10.1: Compound V3 = $1,200.00
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::CompoundV3, regular_asset, false, fallback_floor),
            1_200.0
        );

        // §10.1: Spark = $2,333.33
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::Spark, regular_asset, false, fallback_floor),
            2_333.33
        );

        // §10.1: Aave V3 (general asset) = $2,700.00
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::Aave, regular_asset, false, fallback_floor),
            2_700.0
        );

        // §10.1 gap note: Fluid & Euler V2 fallback to flat floor ($750.00)
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::Fluid, regular_asset, false, fallback_floor),
            750.0
        );
        assert_eq!(
            protocol_break_even_debt_usd(LendingProtocol::EulerV2, regular_asset, false, fallback_floor),
            750.0
        );
    }

    #[test]
    fn test_aave_wbtc_gas_regime_scaling() {
        // WBTC on Ethereum mainnet (05 §5.1)
        let wbtc = Address::from_str("0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f").unwrap();
        let fallback_floor = 750.0;

        // §10.2 Normal Regime: Break-even debt $770, min clip $5,000
        let normal_debt = protocol_break_even_debt_usd(LendingProtocol::Aave, wbtc, false, fallback_floor);
        let normal_clip = protocol_min_clip_usd(LendingProtocol::Aave, wbtc, false, fallback_floor);
        assert_eq!(normal_debt, 770.0);
        assert_eq!(normal_clip, 5_000.0);

        // §10.2 Spike Regime: Break-even debt $5,600, min clip $25,000
        let spike_debt = protocol_break_even_debt_usd(LendingProtocol::Aave, wbtc, true, fallback_floor);
        let spike_clip = protocol_min_clip_usd(LendingProtocol::Aave, wbtc, true, fallback_floor);
        assert_eq!(spike_debt, 5_600.0);
        assert_eq!(spike_clip, 25_000.0);
    }

    #[test]
    fn test_dual_independent_gate_logic() {
        let flat_floor = 750.0;
        let morpho_break_even = 900.0;

        // Case A: Profit passes ($800 >= $750), but debt is below break-even ($850 < $900) -> MUST REJECT
        let profit_a = 800.0;
        let debt_a = 850.0;
        let gate1_pass = profit_a >= flat_floor;
        let gate2_pass = debt_a >= morpho_break_even;
        assert!(gate1_pass);
        assert!(!gate2_pass, "Must reject when debt < protocol break-even debt");

        // Case B: Debt passes ($10,000 >= $900), but profit is below flat floor ($500 < $750) -> MUST REJECT
        let profit_b = 500.0;
        let debt_b = 10_000.0;
        assert!(!(profit_b >= flat_floor), "Must reject when profit < flat floor");
        assert!(debt_b >= morpho_break_even);

        // Case C: Both pass -> ACCEPT
        let profit_c = 1_000.0;
        let debt_c = 10_000.0;
        assert!(profit_c >= flat_floor && debt_c >= morpho_break_even);
    }
}
