//! Trader Joe SwapVenueAdapter (Avalanche C-Chain).
//! 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct TraderJoeAdapter {
    pub router: Address,
}

impl TraderJoeAdapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for TraderJoeAdapter {
    fn default() -> Self {
        Self {
            router: "0xb43133ea024a93947245d8b9449f8749a175f73d".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for TraderJoeAdapter {
    fn id(&self) -> &'static str {
        "trader_joe"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Trader Joe LB baseline 20 bps
        Ok(amount_in * 998 / 1000)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // swapExactTokensForTokens: 0x38ed1739
        let sel = [0x38, 0xed, 0x17, 0x39];
        let path = vec![Token::Address(token_in), Token::Address(token_out)];
        let params = encode(&[
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Array(path),
            Token::Address(Address::zero()),
            Token::Uint(U256::from(u64::MAX)),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
