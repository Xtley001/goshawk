//! HyperCore L1Read precompile client for HyperEVM.
//! 05_CHAIN_HYPEREVM.md, ported per 02_SKUA_STRIP §7.
//!
//! HyperCore prices change every block. Never cache across block boundaries.
//! L1READ precompile address: 0x0000000000000000000000000000000000000800

use anyhow::{anyhow, Result};
use ethers::abi::{decode, encode, ParamType, Token};
use ethers::prelude::*;
use ethers::types::{Address, Bytes, TransactionRequest, U256};
use std::sync::Arc;

pub const L1_READ_PRECOMPILE: &str = "0x0000000000000000000000000000000000000800";

pub struct PrecompileReader<M> {
    pub client: Arc<M>,
    pub precompile_addr: Address,
}

impl<M: Middleware + 'static> PrecompileReader<M> {
    pub fn new(client: Arc<M>) -> Self {
        Self {
            client,
            precompile_addr: L1_READ_PRECOMPILE.parse().expect("precompile address valid"),
        }
    }

    /// Read oracle/mark price for a token index.
    /// Scaled by 10^8: human_price * 1e8.
    pub async fn oracle_px(&self, token_index: u32) -> Result<u64> {
        // oraclePx(uint32) selector: 0x6e9f29ee
        let sel = &ethers::utils::keccak256(b"oraclePx(uint32)")[..4];
        let calldata = [sel, &encode(&[Token::Uint(U256::from(token_index))])].concat();

        let req = TransactionRequest {
            to:   Some(self.precompile_addr.into()),
            data: Some(calldata.into()),
            ..Default::default()
        };

        let res = self.client.call(&req.into(), None).await?;
        if res.is_empty() {
            return Err(anyhow!("Empty response from L1Read.oraclePx({})", token_index));
        }

        let tokens = decode(&[ParamType::Uint(64)], &res)?;
        let px = tokens.first()
            .and_then(|t| t.clone().into_uint())
            .ok_or_else(|| anyhow!("Failed to decode uint64 from oraclePx"))?
            .as_u64();

        if px == 0 {
            return Err(anyhow!("L1Read.oraclePx({}) returned 0", token_index));
        }
        Ok(px)
    }

    /// Read spot mid-price for a market index.
    pub async fn spot_px(&self, market_index: u32) -> Result<u64> {
        // spotPx(uint32) selector
        let sel = &ethers::utils::keccak256(b"spotPx(uint32)")[..4];
        let calldata = [sel, &encode(&[Token::Uint(U256::from(market_index))])].concat();

        let req = TransactionRequest {
            to:   Some(self.precompile_addr.into()),
            data: Some(calldata.into()),
            ..Default::default()
        };

        let res = self.client.call(&req.into(), None).await?;
        let tokens = decode(&[ParamType::Uint(64)], &res)?;
        let px = tokens.first()
            .and_then(|t| t.clone().into_uint())
            .ok_or_else(|| anyhow!("Failed to decode uint64 from spotPx"))?
            .as_u64();

        if px == 0 {
            return Err(anyhow!("L1Read.spotPx({}) returned 0", market_index));
        }
        Ok(px)
    }

    /// Read spot balance of a user for a token index.
    pub async fn spot_balance(&self, user: Address, token_index: u32) -> Result<u64> {
        let sel = &ethers::utils::keccak256(b"spotBalance(address,uint32)")[..4];
        let calldata = [sel, &encode(&[Token::Address(user), Token::Uint(U256::from(token_index))])].concat();

        let req = TransactionRequest {
            to:   Some(self.precompile_addr.into()),
            data: Some(calldata.into()),
            ..Default::default()
        };

        let res = self.client.call(&req.into(), None).await?;
        let tokens = decode(&[ParamType::Uint(64)], &res)?;
        let bal = tokens.first()
            .and_then(|t| t.clone().into_uint())
            .ok_or_else(|| anyhow!("Failed to decode uint64 from spotBalance"))?
            .as_u64();
        Ok(bal)
    }
}
