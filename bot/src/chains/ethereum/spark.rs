//! Spark LendingMarketAdapter for Ethereum mainnet.
//! 08_CHAIN_ETHEREUM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::BorrowPosition;

// TODO(GAP): Spark SparkLend Pool proxy address is NOT SOURCED (05_PROTOCOLS_AND_ADDRESSES.md §5.3, 12_RULES.md §12.5)
pub const SPARK_POOL_PROXY: &str = ethereum::SPARK_POOL_PROXY;

pub struct SparkAdapter {
    pub addresses_provider: Address,
    pub pool_address:       Address,
    pub positions:          Arc<RwLock<Vec<BorrowPosition>>>,
}

impl SparkAdapter {
    pub fn new(addresses_provider: Address, pool_address: Address) -> Self {
        Self {
            addresses_provider,
            pool_address,
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

impl Default for SparkAdapter {
    fn default() -> Self {
        Self {
            addresses_provider: Address::zero(),
            pool_address:       Address::zero(),
            positions:          Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for SparkAdapter {
    fn id(&self) -> &'static str {
        "spark"
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
        pos: &BorrowPosition,
        route: SwapRoute,
        _min_profit: U256,
    ) -> Result<Bytes> {
        // Spark is an Aave V3 fork — uses same liquidationCall signature: 0x00a718a9
        let sel = [0x00, 0xa7, 0x18, 0xa9];
        let params = encode(&[
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Address(pos.borrower),
            Token::Uint(pos.debt_amount / 2),
            Token::Bool(false),
        ]);
        let liquidation_call = [sel.as_slice(), params.as_slice()].concat();

        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(3u8)), // protocol_id 3 = Spark
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount / 2),
            Token::Bytes(liquidation_call),
            Token::Address(Address::zero()),
            Token::Bytes(route.path.to_vec()),
        ])]);

        Ok(Bytes::from(strat_data))
    }
}
