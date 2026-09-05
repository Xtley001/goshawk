//! Curve SwapVenueAdapter (Arbitrum, Polygon, Avalanche, Gnosis, Ethereum).
//! 07_CHAIN_AAVE_V3_STANDARD.md, 08_CHAIN_ETHEREUM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct CurveAdapter {
    pub pool: Address,
}

impl CurveAdapter {
    pub fn new(pool: Address) -> Self {
        Self { pool }
    }
}

impl Default for CurveAdapter {
    fn default() -> Self {
        Self {
            pool: Address::zero(),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for CurveAdapter {
    fn id(&self) -> &'static str {
        "curve"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Curve stable pool fee: ~4 bps (0.04%)
        Ok(amount_in * 9996 / 10000)
    }

    fn build_swap_call(
        &self,
        _token_in: Address,
        _token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // exchange(int128 i, int128 j, uint256 dx, uint256 min_dy)
        // selector: 0x3df02124
        let sel = [0x3d, 0xf0, 0x21, 0x24];
        let params = encode(&[
            Token::Int(U256::zero()), // i = 0 (token in index)
            Token::Int(U256::one()),  // j = 1 (token out index)
            Token::Uint(amount_in),
            Token::Uint(min_out),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
