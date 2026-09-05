//! Compound V3 LendingMarketAdapter for Ethereum mainnet.
//! 03_ADAPTER_ARCHITECTURE.md §3.4, 05_PROTOCOLS_AND_ADDRESSES.md §5.5

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::BorrowPosition;

// Risk parameters from 05_PROTOCOLS_AND_ADDRESSES.md §5.5
pub const MAX_LTV: f64 = 0.77;
pub const LIQUIDATION_THRESHOLD: f64 = 0.82;
pub const LIQUIDATION_BONUS: f64 = 0.07;
pub const CLOSE_FACTOR: f64 = 1.00;

// TODO(GAP): Compound V3 Comet proxy address is NOT SOURCED (05 §5.5, 12 §12.5)
pub const COMPOUND_V3_COMET: &str = ethereum::COMPOUND_V3_COMET;

pub struct CompoundV3Adapter {
    pub comet_proxy: Address,
    pub positions:   Arc<RwLock<Vec<BorrowPosition>>>,
}

impl CompoundV3Adapter {
    pub fn new(comet_proxy: Address) -> Self {
        Self {
            comet_proxy,
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
        !self.comet_proxy.is_zero()
    }
}

impl Default for CompoundV3Adapter {
    fn default() -> Self {
        let addr: Address = COMPOUND_V3_COMET.parse().unwrap_or_default();
        Self::new(addr)
    }
}

#[async_trait]
impl LendingMarketAdapter for CompoundV3Adapter {
    fn id(&self) -> &'static str {
        "compound_v3"
    }

    fn oracle_kind(&self) -> OracleKind {
        OracleKind::Chainlink
    }

    async fn refresh_positions(&self) -> Result<()> {
        Ok(())
    }

    async fn positions_below_hf(&self, threshold: f64) -> Vec<BorrowPosition> {
        // Inert/disabled state: if Comet proxy address is not sourced (zero), report zero positions
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
            anyhow::bail!("Compound V3 adapter is disabled: COMPOUND_V3_COMET address is an unconfigured GAP");
        }

        // absorb(address absorber, address[] accounts) per 03 §3.4 & 05 §5.5
        let sel = &ethers::utils::keccak256(b"absorb(address,address[])")[..4];
        let params = encode(&[
            Token::Address(Address::zero()), // filled by executor contract
            Token::Array(vec![Token::Address(pos.borrower)]),
        ]);
        let liquidation_call = [sel, params.as_slice()].concat();

        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(5u8)), // protocol_id 5 = Compound V3
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
