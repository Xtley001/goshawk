//! Uniswap V3 SwapVenueAdapter (Generic across Arbitrum, Optimism, Polygon, Avalanche, Ethereum).
//! 03_ADAPTER_ARCHITECTURE.md, 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct GenericUniswapV3Adapter {
    pub router: Address,
}

impl GenericUniswapV3Adapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for GenericUniswapV3Adapter {
    fn default() -> Self {
        Self {
            router: Address::zero(),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for GenericUniswapV3Adapter {
    fn id(&self) -> &'static str {
        "uniswap_v3"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Standard Uniswap V3 quote estimate with 30 bps fee
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
            Token::Uint(U256::from(3000)), // 0.3% fee
            Token::Address(Address::zero()),
            Token::Uint(U256::from(u64::MAX)),
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Uint(U256::zero()),
        ])]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
