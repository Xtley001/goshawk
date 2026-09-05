//! Morpho Blue LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.6, 05_PROTOCOLS_AND_ADDRESSES.md §5.2

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::{BorrowPosition, LendingProtocol, PositionIndexer};

pub struct MorphoBlueAdapter {
    pub morpho_address:  Address,
    pub indexer:         Option<Arc<PositionIndexer>>,
    pub local_positions: Arc<RwLock<Vec<BorrowPosition>>>,
}

impl MorphoBlueAdapter {
    pub fn new(morpho_address: Address) -> Self {
        Self {
            morpho_address,
            indexer: None,
            local_positions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_indexer(morpho_address: Address, indexer: Option<Arc<PositionIndexer>>) -> Self {
        Self {
            morpho_address,
            indexer,
            local_positions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_positions(&self, positions: Vec<BorrowPosition>) {
        let lock = self.local_positions.clone();
        tokio::spawn(async move {
            *lock.write().await = positions;
        });
    }
}

impl Default for MorphoBlueAdapter {
    fn default() -> Self {
        let addr: Address = ethereum::MORPHO_BLUE.parse().unwrap_or_default();
        Self::new(addr)
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
        if let Some(indexer) = &self.indexer {
            let _ = indexer.position_count();
        }
        Ok(())
    }

    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition> {
        if let Some(indexer) = &self.indexer {
            indexer.positions_below_hf(threshold)
                .into_iter()
                .filter(|p| matches!(p.protocol, LendingProtocol::Morpho))
                .collect()
        } else {
            let lock = self.local_positions.read().await;
            lock.iter()
                .filter(|p| p.health_factor < threshold && matches!(p.protocol, LendingProtocol::Morpho))
                .cloned()
                .collect()
        }
    }

    async fn build_liquidation_calldata(
        &self,
        pos: &BorrowPosition,
        route: SwapRoute,
        _min_profit: U256,
    ) -> Result<Bytes> {
        // Rejection gate per 11_BUILD_ORDER.md Step 4 & 03_ADAPTER_ARCHITECTURE.md §3.6:
        // Morpho Blue is market-ID-keyed. Positions with empty/unresolved or all-zero
        // MarketParams or zero market_id must be strictly rejected.
        if pos.morpho_market_params.is_empty()
            || pos.morpho_market_params.iter().all(|&b| b == 0)
            || pos.morpho_market_id.is_zero()
        {
            anyhow::bail!("Morpho Blue position has unresolved or zero MarketParams — cannot liquidate");
        }

        let router_addr: Address = ethereum::UNISWAP_V3_ROUTER.parse().unwrap_or(Address::zero());
        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(0u8)), // protocol_id 0 = Morpho Blue
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount), // 100% close factor
            Token::Bytes(pos.morpho_market_params.to_vec()),
            Token::Address(router_addr),
            Token::Bytes(route.path.to_vec()),
        ])]);
        Ok(Bytes::from(strat_data))
    }
}
