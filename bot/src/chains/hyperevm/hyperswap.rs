//! HyperSwap SwapVenueAdapter for HyperEVM.
//! 05_CHAIN_HYPEREVM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::swap::SwapVenueAdapter;

pub struct HyperSwapAdapter {
    pub router: Address,
}

impl HyperSwapAdapter {
    pub fn new(router: Address) -> Self {
        Self { router }
    }
}

impl Default for HyperSwapAdapter {
    fn default() -> Self {
        Self {
            router: Address::zero(),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for HyperSwapAdapter {
    fn id(&self) -> &'static str {
        "hyperswap_v2"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // HyperSwap V2 30 bps fee
        Ok(amount_in * 997 / 1000)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
        // selector: 0x38ed1739
        let sel = [0x38, 0xed, 0x17, 0x39];
        let path = vec![Token::Address(token_in), Token::Address(token_out)];
        let params = encode(&[
            Token::Uint(amount_in),
            Token::Uint(min_out),
            Token::Array(path),
            Token::Address(Address::zero()), // receiver set by executor
            Token::Uint(U256::from(u64::MAX)), // deadline
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
