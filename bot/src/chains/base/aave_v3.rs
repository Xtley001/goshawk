//! Aave V3 LendingMarketAdapter for Base.
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

pub struct BaseAaveV3Adapter {
    pub pool_address:     Address,
    pub indexer:          Option<Arc<PositionIndexer>>,
    pub local_positions:  Arc<RwLock<Vec<BorrowPosition>>>,
}

impl BaseAaveV3Adapter {
    pub fn new(pool_address: Address, indexer: Option<Arc<PositionIndexer>>) -> Self {
        Self {
            pool_address,
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

impl Default for BaseAaveV3Adapter {
    fn default() -> Self {
        Self {
            pool_address:    Address::zero(),
            indexer:         None,
            local_positions: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for BaseAaveV3Adapter {
    fn id(&self) -> &'static str {
        "aave_v3"
    }

    fn oracle_kind(&self) -> OracleKind {
        OracleKind::Chainlink
    }

    async fn refresh_positions(&self) -> Result<()> {
        if let Some(indexer) = &self.indexer {
            // Indexer runs background sync loop
            let _ = indexer.position_count();
        }
        Ok(())
    }

    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition> {
        if let Some(indexer) = &self.indexer {
            indexer.positions_below_hf(threshold)
                .into_iter()
                .filter(|p| matches!(p.protocol, LendingProtocol::Aave))
                .collect()
        } else {
            let lock = self.local_positions.read().await;
            lock.iter()
                .filter(|p| p.health_factor < threshold && matches!(p.protocol, LendingProtocol::Aave))
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
            Token::Uint(U256::from(1u8)), // protocol_id 1 = Aave
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount),
            Token::Bytes(vec![]),
            Token::Address(uni_router),
            Token::Bytes(route.path.to_vec()),
        ])]);
        Ok(Bytes::from(strat_data))
    }
}
