//! Fluid LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.3, 05_PROTOCOLS_AND_ADDRESSES.md §5.4

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::BorrowPosition;

// Risk parameters from 05_PROTOCOLS_AND_ADDRESSES.md §5.4
pub const MAX_LTV: f64 = 0.85;
pub const LIQUIDATION_THRESHOLD: f64 = 0.90;
pub const LIQUIDATION_BONUS: f64 = 0.03;
pub const CLOSE_FACTOR: f64 = 1.00;

// TODO(GAP): Fluid Liquidity Layer contract address is NOT SOURCED (05 §5.4, 12 §12.5)
pub const FLUID_LIQUIDITY_LAYER: &str = ethereum::FLUID_LIQUIDITY_LAYER;

pub struct FluidAdapter {
    pub liquidity_layer: Address,
    pub positions:       Arc<RwLock<Vec<BorrowPosition>>>,
}

impl FluidAdapter {
    pub fn new(liquidity_layer: Address) -> Self {
        Self {
            liquidity_layer,
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
        !self.liquidity_layer.is_zero()
    }
}

impl Default for FluidAdapter {
    fn default() -> Self {
        let addr: Address = FLUID_LIQUIDITY_LAYER.parse().unwrap_or_default();
        Self::new(addr)
    }
}

#[async_trait]
impl LendingMarketAdapter for FluidAdapter {
    fn id(&self) -> &'static str {
        "fluid"
    }

    fn oracle_kind(&self) -> OracleKind {
        OracleKind::Chainlink
    }

    async fn refresh_positions(&self) -> Result<()> {
        Ok(())
    }

    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition> {
        // Inert/disabled state: if contract address is not sourced (zero), report zero positions
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
            anyhow::bail!("Fluid adapter is disabled: FLUID_LIQUIDITY_LAYER address is an unconfigured GAP");
        }

        // liquidate(address collateral, address debt, address user, uint256 debtToCover)
        let sel = &ethers::utils::keccak256(b"liquidate(address,address,address,uint256)")[..4];
        let params = encode(&[
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Address(pos.borrower),
            Token::Uint(pos.debt_amount), // 100% close factor
        ]);
        let liquidation_call = [sel, params.as_slice()].concat();

        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(4u8)), // protocol_id 4 = Fluid
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
