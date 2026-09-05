//! Ethereum Aave V3 LendingMarketAdapter with mandatory SVR (Smart Value Recapture) exclusion.
//! 08_CHAIN_ETHEREUM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
use crate::shared::addresses::ethereum;
use crate::shared::position_indexer::BorrowPosition;

// Sourced addresses from 05_PROTOCOLS_AND_ADDRESSES.md §5.1
pub const AAVE_V3_POOL_PROXY:         &str = ethereum::AAVE_V3_POOL_PROXY;
pub const AAVE_V3_PRICE_ORACLE:       &str = ethereum::AAVE_V3_PRICE_ORACLE;
pub const AAVE_V3_WBTC_UNDERLYING:    &str = ethereum::AAVE_V3_WBTC_UNDERLYING;
pub const AAVE_V3_AARBWTC:            &str = ethereum::AAVE_V3_AARBWTC;
pub const AAVE_V3_VARIABLE_DEBT:      &str = ethereum::AAVE_V3_VARIABLE_DEBT;

pub struct EthereumAaveV3Adapter {
    pub addresses_provider:   Address,
    pub pool_address:         Address,
    pub svr_excluded_assets:  HashSet<Address>,
    pub positions:            Arc<RwLock<Vec<BorrowPosition>>>,
}

impl EthereumAaveV3Adapter {
    pub fn new(addresses_provider: Address, pool_address: Address) -> Self {
        let mut svr = HashSet::new();
        // Mandatory SVR-enabled markets on Ethereum mainnet per 08_CHAIN_ETHEREUM.md
        // tBTC, LBTC, AAVE, LINK
        let defaults = [
            "0x18084fbA666a33d37592fA2633fD49a74DD93a88", // tBTC
            "0x8236a87084f8B84306f72007F36F2618A5634494", // LBTC
            "0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9", // AAVE
            "0x514910771AF9Ca656af840dff83E8264EcF986CA", // LINK
        ];
        for a in defaults {
            if let Ok(addr) = Address::from_str(a) {
                svr.insert(addr);
            }
        }

        Self {
            addresses_provider,
            pool_address,
            svr_excluded_assets: svr,
            positions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_exclusions(mut self, exclusions: Vec<Address>) -> Self {
        self.svr_excluded_assets.extend(exclusions);
        self
    }

    pub fn is_svr_excluded(&self, collateral: Address, debt: Address) -> bool {
        self.svr_excluded_assets.contains(&collateral) || self.svr_excluded_assets.contains(&debt)
    }

    pub fn set_positions(&self, positions: Vec<BorrowPosition>) {
        let lock = self.positions.clone();
        tokio::spawn(async move {
            *lock.write().await = positions;
        });
    }
}

impl Default for EthereumAaveV3Adapter {
    fn default() -> Self {
        let pool: Address = AAVE_V3_POOL_PROXY.parse().unwrap_or_default();
        Self::new(pool, pool)
    }
}

#[async_trait]
impl LendingMarketAdapter for EthereumAaveV3Adapter {
    fn id(&self) -> &'static str {
        "aave_v3_ethereum"
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
            .filter(|p| {
                p.health_factor < threshold && !self.is_svr_excluded(p.collateral_asset, p.debt_asset)
            })
            .cloned()
            .collect()
    }

    async fn build_liquidation_calldata(
        &self,
        pos: &BorrowPosition,
        route: SwapRoute,
        _min_profit: U256,
    ) -> Result<Bytes> {
        if self.is_svr_excluded(pos.collateral_asset, pos.debt_asset) {
            anyhow::bail!("Position contains SVR-excluded asset — aborting liquidation");
        }

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
            Token::Uint(U256::from(1u8)),
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
