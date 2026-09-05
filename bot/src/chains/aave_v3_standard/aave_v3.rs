//! Standard Aave V3 LendingMarketAdapter for 6 chains:
//! Arbitrum, Optimism, Polygon, Avalanche, BNB Chain, Gnosis Chain.
//! 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::BorrowPosition;

pub struct StandardAaveV3Adapter {
    pub chain_name:       String,
    pub addresses_provider: Address,
    pub pool_address:     Address,
    pub positions:        Arc<RwLock<Vec<BorrowPosition>>>,
}

impl StandardAaveV3Adapter {
    pub fn new(chain_name: impl Into<String>, addresses_provider: Address, pool_address: Address) -> Self {
        Self {
            chain_name:       chain_name.into(),
            addresses_provider,
            pool_address,
            positions:        Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_positions(&self, positions: Vec<BorrowPosition>) {
        let lock = self.positions.clone();
        tokio::spawn(async move {
            *lock.write().await = positions;
        });
    }
}

impl Default for StandardAaveV3Adapter {
    fn default() -> Self {
        Self {
            chain_name:       "standard_aave_v3".into(),
            addresses_provider: Address::zero(),
            pool_address:     Address::zero(),
            positions:        Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for StandardAaveV3Adapter {
    fn id(&self) -> &'static str {
        "aave_v3"
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
        // Standard Aave V3 liquidationCall(collateralAsset, debtAsset, user, debtToCover, receiveAToken)
        // selector: 0x00a718a9
        let sel = [0x00, 0xa7, 0x18, 0xa9];
        let params = encode(&[
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Address(pos.borrower),
            Token::Uint(pos.debt_amount / 2), // Standard 50% close factor
            Token::Bool(false), // receive underlying
        ]);
        let liquidation_call = [sel.as_slice(), params.as_slice()].concat();

        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(1u8)), // protocol_id 1 = Aave V3
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount / 2),
            Token::Bytes(liquidation_call),
            Token::Address(Address::zero()), // router
            Token::Bytes(route.path.to_vec()),
        ])]);

        Ok(Bytes::from(strat_data))
    }
}
