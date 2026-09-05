//! QuickSwap SwapVenueAdapter (Polygon PoS).
//! 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct QuickSwapAdapter {
    pub router: Address,
}

impl QuickSwapAdapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for QuickSwapAdapter {
    fn default() -> Self {
        Self {
            router: "0xf5b509bB0909a69B1c207E495f687a596C168E12".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for QuickSwapAdapter {
    fn id(&self) -> &'static str {
        "quickswap"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // QuickSwap Algebra dynamic fee baseline 30 bps
        Ok(amount_in * 997 / 1000)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // exactInputSingle selector: 0x414bf389
        let sel = [0x41, 0x4b, 0xf3, 0x89];
        let params = encode(&[Token::Tuple(vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Uint(U256::from(3000)),
            Token::Address(Address::zero()),
            Token::Uint(U256::from(u64::MAX)),
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Uint(U256::zero()),
        ])]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
