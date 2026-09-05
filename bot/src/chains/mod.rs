//! Cross-chain liquidation adapters and boot-time gate enforcement.
//! Per 03_ADAPTER_ARCHITECTURE.md:
//! One Strategy, N Venues. LendingMarketAdapter, FlashLoanAdapter, SwapVenueAdapter.

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::shared::position_indexer::BorrowPosition;

pub mod ethereum;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleKind {
    Chainlink,
    HyperCorePrecompile,
    /// Spot AMM pricing is strictly forbidden for liquidation eligibility gating.
    SpotAmm,
}

#[derive(Debug, Clone)]
pub struct SwapRoute {
    pub venue_id:     String,
    pub path:         Bytes,
    pub expected_out: U256,
}

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

#[derive(Default)]
pub struct LendingMarketRegistry {
    markets: HashMap<String, Box<dyn LendingMarketAdapter>>,
}

impl LendingMarketRegistry {
    pub fn new() -> Self {
        Self { markets: HashMap::new() }
    }

    pub fn insert(&mut self, id: &str, adapter: Box<dyn LendingMarketAdapter>) {
        self.markets.insert(id.to_string(), adapter);
    }

    pub fn get(&self, id: &str) -> Option<&(dyn LendingMarketAdapter + 'static)> {
        self.markets.get(id).map(|b| b.as_ref())
    }

    pub fn len(&self) -> usize {
        self.markets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.markets.is_empty()
    }
}

/// Boot-time oracle gate enforcement.
/// Refuses registration if adapter reports OracleKind::SpotAmm.
pub fn register_lending_market(
    registry: &mut LendingMarketRegistry,
    adapter: Box<dyn LendingMarketAdapter>,
) -> Result<()> {
    if adapter.oracle_kind() == OracleKind::SpotAmm {
        anyhow::bail!(
            "refusing to register {}: SpotAmm-gated oracle — liquidation \
             eligibility cannot be gated off a same-block-manipulable price. \
             This is not a config error to work around; it means the market \
             doesn't belong in this system.",
            adapter.id()
        );
    }
    registry.insert(adapter.id(), adapter);
    Ok(())
}

/// Per-chain adaptive circuit breaker tracking reverts, consecutive successes, and accumulated revert cost.
#[derive(Debug)]
pub struct ChainBreaker {
    pub reverts:              AtomicU32,
    pub consec:               AtomicU32,
    pub threshold:            u32,
    pub accumulated_loss_usd: parking_lot::RwLock<f64>,
    pub max_loss_usd:         f64,
}

impl ChainBreaker {
    pub fn new(threshold: u32) -> Self {
        let max_loss = (threshold as f64) * 10.0;
        Self {
            reverts:              AtomicU32::new(0),
            consec:               AtomicU32::new(0),
            threshold,
            accumulated_loss_usd: parking_lot::RwLock::new(0.0),
            max_loss_usd:         max_loss,
        }
    }

    pub fn with_max_loss(threshold: u32, max_loss_usd: f64) -> Self {
        Self {
            reverts:              AtomicU32::new(0),
            consec:               AtomicU32::new(0),
            threshold,
            accumulated_loss_usd: parking_lot::RwLock::new(0.0),
            max_loss_usd,
        }
    }

    pub fn is_tripped(&self) -> bool {
        let count_tripped = self.reverts.load(Ordering::SeqCst) >= self.threshold;
        let cost_tripped = *self.accumulated_loss_usd.read() >= self.max_loss_usd;
        count_tripped || cost_tripped
    }

    pub fn record_revert(&self) {
        self.record_revert_with_cost(1.0);
    }

    /// Record revert with realized gas cost:
    /// High-cost reverts add additional penalty weight to trip the breaker faster.
    pub fn record_revert_with_cost(&self, cost_usd: f64) {
        let penalty_weight = (1.0 + (cost_usd / 5.0)).min(5.0) as u32;
        self.reverts.fetch_add(penalty_weight.max(1), Ordering::SeqCst);
        self.consec.store(0, Ordering::SeqCst);

        let mut loss = self.accumulated_loss_usd.write();
        *loss += cost_usd.max(0.0);
    }

    pub fn record_success(&self) {
        let prev = self.consec.fetch_add(1, Ordering::SeqCst);
        if prev + 1 >= 3 {
            self.reverts.store(0, Ordering::SeqCst);
            *self.accumulated_loss_usd.write() = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSpotAmmAdapter;
    #[async_trait]
    impl LendingMarketAdapter for MockSpotAmmAdapter {
        fn id(&self) -> &'static str { "mock_spot_amm" }
        fn oracle_kind(&self) -> OracleKind { OracleKind::SpotAmm }
        async fn refresh_positions(&self) -> Result<()> { Ok(()) }
        async fn positions_below_hf(&self, _threshold: f64) -> Vec<BorrowPosition> { vec![] }
        async fn build_liquidation_calldata(&self, _pos: &BorrowPosition, _route: SwapRoute, _min_profit: U256) -> Result<Bytes> {
            Ok(Bytes::new())
        }
    }

    struct MockChainlinkAdapter;
    #[async_trait]
    impl LendingMarketAdapter for MockChainlinkAdapter {
        fn id(&self) -> &'static str { "mock_chainlink" }
        fn oracle_kind(&self) -> OracleKind { OracleKind::Chainlink }
        async fn refresh_positions(&self) -> Result<()> { Ok(()) }
        async fn positions_below_hf(&self, _threshold: f64) -> Vec<BorrowPosition> { vec![] }
        async fn build_liquidation_calldata(&self, _pos: &BorrowPosition, _route: SwapRoute, _min_profit: U256) -> Result<Bytes> {
            Ok(Bytes::new())
        }
    }

    #[test]
    fn test_spot_amm_rejection_gate() {
        let mut registry = LendingMarketRegistry::new();
        let res = register_lending_market(&mut registry, Box::new(MockSpotAmmAdapter));
        assert!(res.is_err(), "SpotAmm adapter MUST be rejected at registration");
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("SpotAmm-gated oracle"));
        assert_eq!(registry.len(), 0);

        let res_ok = register_lending_market(&mut registry, Box::new(MockChainlinkAdapter));
        assert!(res_ok.is_ok(), "Chainlink adapter MUST be accepted");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_adaptive_circuit_breaker() {
        // High-cost revert trips threshold much faster
        let breaker = ChainBreaker::new(10);
        assert!(!breaker.is_tripped());

        // Low cost revert ($0.02 L2 style) increments counter by 1
        breaker.record_revert_with_cost(0.02);
        assert!(!breaker.is_tripped());

        // Expensive revert ($25 mainnet style) adds heavy penalty weight
        breaker.record_revert_with_cost(25.0);
        // Penalty weight is 5, so reverts counter is at least 6 now
        assert!(breaker.reverts.load(Ordering::SeqCst) >= 6);

        // Another expensive revert trips the breaker immediately
        breaker.record_revert_with_cost(30.0);
        assert!(breaker.is_tripped());

        // 3 consecutive successes reset the breaker
        breaker.record_success();
        breaker.record_success();
        breaker.record_success();
        assert!(!breaker.is_tripped());
    }
}
