//! Euler V2 LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.5, 05_PROTOCOLS_AND_ADDRESSES.md §5.6

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::BorrowPosition;

// TODO(GAP): Euler V2 VaultController address is NOT SOURCED (see 12_RULES.md §12.5)
pub const EULER_V2_VAULT_CONTROLLER: &str = "";

pub struct EulerV2Adapter {
    pub vault_controller: Address,
    pub positions:        Arc<RwLock<Vec<BorrowPosition>>>,
}

impl EulerV2Adapter {
    pub fn new(vault_controller: Address) -> Self {
        Self {
            vault_controller,
            positions: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for EulerV2Adapter {
    fn default() -> Self {
        Self {
            vault_controller: Address::zero(),
            positions:        Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for EulerV2Adapter {
    fn id(&self) -> &'static str {
        "euler_v2"
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
