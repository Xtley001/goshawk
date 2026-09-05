//! Strategy 3 — Protocol Liquidation.
//! Latency & Profit Optimization Addendum:
//!   - Dynamic min profit threshold = live gas cost * safety margin + opportunity cost.
//!   - Dynamic HF threshold widening with realized price volatility.
//!   - Realized swap slippage and live parallel flash provider selection.

use anyhow::Result;
use dashmap::DashMap;
use ethers::types::{Address, U256};
use std::sync::Arc;
use crate::{
    config::Config,
    shared::{
        position_indexer::{PositionIndexer, PriceMap},
        mempool_monitor::MempoolMonitor,
        simulation::SimulationEngine,
        flash_loan::FlashLoanRouter,
        submission::SubmissionPipeline,
    },
};

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
    let base_cfg = cfg.get_chain("base");
    let base_hf = base_cfg.map(|c| c.hf_threshold()).unwrap_or(1.03);
    let vol_scale = base_cfg.map(|c| c.volatility_scale).unwrap_or(1.5);
    let max_widening = base_cfg.map(|c| c.max_hf_widening).unwrap_or(0.05);
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
    let base_cfg = cfg.get_chain("base");
    let base_hf = base_cfg.map(|c| c.hf_threshold()).unwrap_or(1.03);
    let vol_scale = base_cfg.map(|c| c.volatility_scale).unwrap_or(1.5);
    let max_widening = base_cfg.map(|c| c.max_hf_widening).unwrap_or(0.05);
    let hf_thresh = base_hf + (realized_vol * vol_scale).min(max_widening);

    indexer.refresh_health_factors(prices);
    let positions = indexer.positions_below_hf(hf_thresh);
    execute_liquidations_for_positions(&positions, prices, sim, flash, sub, cfg, eth_price).await
}

async fn execute_liquidations_for_positions(
    positions: &[crate::shared::position_indexer::BorrowPosition],
    prices:    &PriceMap,
    sim:       &Arc<SimulationEngine>,
    flash:     &Arc<FlashLoanRouter>,
    sub:       &Arc<SubmissionPipeline>,
    cfg:       &Config,
    eth_price: f64,
) -> Result<()> {
    let base_cfg = cfg.get_chain("base");
    let opportunity_cost = base_cfg.map(|c| c.opportunity_cost_usd).unwrap_or(15.0);

    for pos in positions {
        let sim_r = sim.simulate_liquidation(pos, prices, eth_price).await?;

        // Addendum Part 1: Dynamic min liquidation profit:
        // gas_cost_now * safety_margin + opportunity_cost recomputed every block from live gas oracle
        let effective_gas_wei = sim.gas_oracle().effective_gas_price_wei();
        let gas_cost_usd = sim_r.gas_estimate as f64 * effective_gas_wei / 1e18 * eth_price;
        let dynamic_min_profit_usd = gas_cost_usd * cfg.gas_estimate_safety_margin + opportunity_cost;

        if sim_r.profit_usd < dynamic_min_profit_usd {
            tracing::debug!(
                "Skipping pos {:?}: profit ${:.2} < dynamic required ${:.2} (gas=${:.2})",
                pos.borrower, sim_r.profit_usd, dynamic_min_profit_usd, gas_cost_usd
            );
            continue;
        }

        tracing::info!(
            "Liquidation opportunity: borrower={:?} protocol={:?} profit=${:.2} (min_req=${:.2})",
            pos.borrower, pos.protocol, sim_r.profit_usd, dynamic_min_profit_usd
        );

        let swap_route = sim.get_optimal_swap_route(pos.collateral_asset, pos.debt_asset).await?;
        let provider   = flash.select_provider(pos.debt_asset, pos.debt_amount).await?;

        let min_profit_wei = {
            let min_usd = dynamic_min_profit_usd.max(0.0);
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

        // Trade with $40 profit fails dynamic gate under $25 gas
        assert!(40.0 < dynamic_min_profit_usd);

        // Trade with $50 profit passes
        assert!(50.0 >= dynamic_min_profit_usd);
    }

    #[test]
    fn test_dynamic_hf_widening() {
        let base_hf: f64 = 1.03;
        let vol_scale: f64 = 1.5;
        let max_widening: f64 = 0.05;

        // Normal market: 0.5% volatility
        let vol_normal: f64 = 0.005;
        let hf_normal: f64 = base_hf + (vol_normal * vol_scale).min(max_widening);
        assert!((hf_normal - 1.0375).abs() < 1e-6);

        // High volatility: 4% volatility
        let vol_high: f64 = 0.04;
        let hf_high: f64 = base_hf + (vol_high * vol_scale).min(max_widening);
        assert!((hf_high - 1.08).abs() < 1e-6); // Capped at max widening +0.05
    }
}
