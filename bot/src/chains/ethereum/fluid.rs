//! Fluid LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.3, 05_PROTOCOLS_AND_ADDRESSES.md §5.4

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::BorrowPosition;

// TODO(GAP): Fluid Liquidity Layer contract address is NOT SOURCED (see 12_RULES.md §12.5)
pub const FLUID_LIQUIDITY_LAYER: &str = "";

pub struct FluidAdapter {
    pub liquidity_layer: Address,
    pub positions:       Arc<RwLock<Vec<BorrowPosition>>>,
}

impl FluidAdapter {
    pub fn new(liquidity_layer: Address) -> Self {
        Self {
            liquidity_layer,
            positions: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for FluidAdapter {
    fn default() -> Self {
        Self {
            liquidity_layer: Address::zero(),
            positions:       Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for FluidAdapter {
    fn id(&self) -> &'static str {
        "fluid"
    }

    fn oracle_kind(&self) -> OracleKind {
        OracleKind::Chainlink
    }

    async fn refresh_positions(&self) -> Result<()> {
        Ok(())
    }

    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition> {
        let lock = self.positions.read().await;
        lock.iter()
            .filter(|p| p.health_factor < threshold)
            .cloned()
            .collect()
    }

    async fn build_liquidation_calldata(
        &self,
        _pos: &BorrowPosition,
        _route: SwapRoute,
        _min_profit: U256,
    ) -> Result<Bytes> {
        // Real implementation in Step 5
        Ok(Bytes::new())
    }
}
