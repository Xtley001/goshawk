//! Morpho Blue LendingMarketAdapter for Base.
//! 06_CHAIN_BASE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::base;
use crate::shared::position_indexer::{BorrowPosition, LendingProtocol, PositionIndexer};

pub struct BaseMorphoBlueAdapter {
    pub morpho_address:   Address,
    pub indexer:          Option<Arc<PositionIndexer>>,
    pub local_positions:  Arc<RwLock<Vec<BorrowPosition>>>,
}

impl BaseMorphoBlueAdapter {
    pub fn new(morpho_address: Address, indexer: Option<Arc<PositionIndexer>>) -> Self {
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

impl Default for BaseMorphoBlueAdapter {
    fn default() -> Self {
        Self {
            morpho_address:  Address::zero(),
            indexer:         None,
            local_positions: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for BaseMorphoBlueAdapter {
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
        let uni_router: Address = base::UNISWAP_V3_ROUTER.parse()?;
        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(0u8)), // protocol_id 0 = Morpho
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount),
            Token::Bytes(pos.morpho_market_params.to_vec()),
            Token::Address(uni_router),
            Token::Bytes(route.path.to_vec()),
        ])]);
        Ok(Bytes::from(strat_data))
    }
}
