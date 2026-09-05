//! HyperLend LendingMarketAdapter for HyperEVM.
//! 05_CHAIN_HYPEREVM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};

pub struct HyperLendAdapter {
    pub pool_address:     Address,
    pub positions:        Arc<RwLock<Vec<BorrowPosition>>>,
}

impl HyperLendAdapter {
    pub fn new(pool_address: Address) -> Self {
        Self {
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

    /// Dynamic close factor: 100% if HF < 0.95, else 50%.
    pub fn close_factor_bps(health_factor: f64) -> u64 {
        if health_factor < 0.95 {
            10_000
        } else {
            5_000
        }
    }

    /// Max liquidatable debt given borrower's total debt and HF.
    pub fn max_liquidatable_debt(debt_amount: U256, health_factor: f64) -> U256 {
        let factor = Self::close_factor_bps(health_factor);
        debt_amount * factor / 10_000
    }
}

impl Default for HyperLendAdapter {
    fn default() -> Self {
        Self {
            pool_address: Address::zero(),
            positions:    Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LendingMarketAdapter for HyperLendAdapter {
    fn id(&self) -> &'static str {
        "hyperlend"
    }

    fn oracle_kind(&self) -> OracleKind {
        OracleKind::HyperCorePrecompile
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
        let debt_to_cover = Self::max_liquidatable_debt(pos.debt_amount, pos.health_factor);
        // liquidationCall(address collateralAsset, address debtAsset, address user, uint256 debtToCover, bool receiveAToken)
        // selector: 0x00a718a9
        let sel = [0x00, 0xa7, 0x18, 0xa9];
        let params = encode(&[
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Address(pos.borrower),
            Token::Uint(debt_to_cover),
            Token::Bool(false), // receiveAToken = false (receive underlying)
        ]);
        let liquidation_call = [sel.as_slice(), params.as_slice()].concat();

        // Wrap in executor liquidation envelope
        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(2u8)), // protocol_id 2 = HyperLend
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(debt_to_cover),
            Token::Bytes(liquidation_call),
            Token::Address(Address::zero()), // router
            Token::Bytes(route.path.to_vec()),
        ])]);

        Ok(Bytes::from(strat_data))
    }
}
