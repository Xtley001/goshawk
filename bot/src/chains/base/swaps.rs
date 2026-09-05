//! Base swap adapters: Aerodrome and Uniswap V3.
//! 06_CHAIN_BASE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct AerodromeAdapter {
    pub router: Address,
}

impl AerodromeAdapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for AerodromeAdapter {
    fn default() -> Self {
        Self {
            router: "0xcF77a3Ba9A5CA399B7c97c748840422BeEgA0274".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for AerodromeAdapter {
    fn id(&self) -> &'static str {
        "aerodrome"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Flat conservative haircut estimate for DEX quote
        Ok(amount_in * 98 / 100)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // Aerodrome swapExactTokensForTokens selector: 0x6e7a43b8
        let sel = [0x6e, 0x7a, 0x43, 0xb8];
        let route = Token::Tuple(vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Bool(false), // volatile pool
            Token::Address(Address::zero()), // default factory
        ]);
        let params = encode(&[
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Array(vec![route]),
            Token::Address(Address::zero()), // receiver set by executor
            Token::Uint(U256::from(u64::MAX)), // deadline
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}

pub struct BaseUniswapV3Adapter {
    pub router: Address,
}

impl BaseUniswapV3Adapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for BaseUniswapV3Adapter {
    fn default() -> Self {
        Self {
            router: "0x2626664c2603336E57B271c5C0b26F421741e481".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for BaseUniswapV3Adapter {
    fn id(&self) -> &'static str {
        "uniswap_v3"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Standard DEX model output
        Ok(amount_in * 982 / 1000)
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
            Token::Uint(U256::from(3000)), // fee tier 30 bps default
            Token::Address(Address::zero()),
            Token::Uint(U256::from(u64::MAX)),
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Uint(U256::zero()),
        ])]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
