//! Euler V2 LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.5, 05_PROTOCOLS_AND_ADDRESSES.md §5.6

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::BorrowPosition;

// Risk parameters from 05_PROTOCOLS_AND_ADDRESSES.md §5.6
pub const MAX_LTV: f64 = 0.88;
pub const LIQUIDATION_THRESHOLD: f64 = 0.92;
pub const LIQUIDATION_BONUS: f64 = 0.06;
pub const CLOSE_FACTOR: f64 = 1.00;

// TODO(GAP): Euler V2 VaultController address is NOT SOURCED (05 §5.6, 12 §12.5)
pub const EULER_V2_VAULT_CONTROLLER: &str = ethereum::EULER_V2_VAULT_CONTROLLER;

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

    pub fn set_positions(&self, positions: Vec<BorrowPosition>) {
        let lock = self.positions.clone();
        tokio::spawn(async move {
            *lock.write().await = positions;
        });
    }

    pub fn is_enabled(&self) -> bool {
        !self.vault_controller.is_zero()
    }
}

impl Default for EulerV2Adapter {
    fn default() -> Self {
        let addr: Address = EULER_V2_VAULT_CONTROLLER.parse().unwrap_or_default();
        Self::new(addr)
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
        // Inert/disabled state: if VaultController address is not sourced (zero), report zero positions
        if !self.is_enabled() {
            return vec![];
        }
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
        if !self.is_enabled() {
            anyhow::bail!("Euler V2 adapter is disabled: EULER_V2_VAULT_CONTROLLER address is an unconfigured GAP");
        }

        // liquidateVault(address collateral, address debt, address borrower, uint256 debtToCover)
        let sel = &ethers::utils::keccak256(b"liquidateVault(address,address,address,uint256)")[..4];
        let params = encode(&[
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Address(pos.borrower),
            Token::Uint(pos.debt_amount),
        ]);
        let liquidation_call = [sel, params.as_slice()].concat();

        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(6u8)), // protocol_id 6 = Euler V2
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount),
            Token::Bytes(liquidation_call),
            Token::Address(Address::zero()),
            Token::Bytes(route.path.to_vec()),
        ])]);

        Ok(Bytes::from(strat_data))
    }
}
