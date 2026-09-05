//! Morpho Blue LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.6, 05_PROTOCOLS_AND_ADDRESSES.md §5.2

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::BorrowPosition;

pub struct MorphoBlueAdapter {
    pub morpho_address: Address,
    pub positions:      Arc<RwLock<Vec<BorrowPosition>>>,
}

impl MorphoBlueAdapter {
    pub fn new(morpho_address: Address) -> Self {
        Self {
            morpho_address,
            positions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_positions(&self, positions: Vec<BorrowPosition>) {
        let lock = self.positions.clone();
        tokio::spawn(async move {
            *lock.write().await = positions;
        });
    }
}

impl Default for MorphoBlueAdapter {
    fn default() -> Self {
        Self {
            morpho_address: Address::zero(),
            positions:      Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for MorphoBlueAdapter {
    fn id(&self) -> &'static str {
        "morpho_blue"
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
        // Real implementation in Step 4
        Ok(Bytes::new())
    }
}
