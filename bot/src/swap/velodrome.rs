//! Velodrome SwapVenueAdapter (Optimism).
//! 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct VelodromeAdapter {
    pub router: Address,
}

impl VelodromeAdapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for VelodromeAdapter {
    fn default() -> Self {
        Self {
            router: "0xa062aE8A9c5e11aaA026fc2670B0D65cCc8B2858".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for VelodromeAdapter {
    fn id(&self) -> &'static str {
        "velodrome"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        Ok(amount_in * 997 / 1000)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // swapExactTokensForTokens: 0x6e7a43b8
        let sel = [0x6e, 0x7a, 0x43, 0xb8];
        let route = Token::Tuple(vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Bool(false), // volatile pool
            Token::Address(Address::zero()), // factory
        ]);
        let params = encode(&[
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Array(vec![route]),
            Token::Address(Address::zero()),
            Token::Uint(U256::from(u64::MAX)),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
