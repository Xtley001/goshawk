//! Plasma adapter module (pure settlement / empty swap venues).
//! 09_CHAIN_PLASMA.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::BorrowPosition;

pub struct PlasmaAaveV3Adapter {
    pub addresses_provider: Address,
    pub pool_address:       Address,
    pub positions:          Arc<RwLock<Vec<BorrowPosition>>>,
}

impl PlasmaAaveV3Adapter {
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

impl Default for PlasmaAaveV3Adapter {
    fn default() -> Self {
        Self {
            addresses_provider: Address::zero(),
            pool_address:       Address::zero(),
            positions:          Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for PlasmaAaveV3Adapter {
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
        // Standard Aave V3 liquidationCall
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
            Token::Uint(U256::from(1u8)), // Aave V3
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
    use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};
    use ethers::types::{Address, Bytes, TxHash, U256};

    #[tokio::test]
    async fn test_plasma_aave_v3_adapter_and_empty_swaps() {
        let adapter = PlasmaAaveV3Adapter::default();
        assert_eq!(adapter.id(), "aave_v3");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(10_000u64),
            collateral_amount: U256::from(20_000u64),
            health_factor: 1.01,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 300,
        };

        adapter.set_positions(vec![pos.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let at_risk = adapter.positions_below_hf(1.03).await;
        assert_eq!(at_risk.len(), 1);

        // Empty swap route: Plasma holds seized collateral for carry / settlement
        let empty_route = SwapRoute {
            venue_id: "".into(),
            path: Bytes::new(),
            expected_out: U256::zero(),
        };

        let cd = adapter.build_liquidation_calldata(&pos, empty_route, U256::zero()).await;
        assert!(cd.is_ok());
        assert!(!cd.unwrap().is_empty());
    }
}
