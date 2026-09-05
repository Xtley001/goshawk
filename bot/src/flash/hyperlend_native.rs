//! HyperLend native FlashLoanAdapter for HyperEVM.
//! 05_CHAIN_HYPEREVM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;

pub struct HyperLendNativeFlashAdapter {
    pub pool: Address,
}

impl HyperLendNativeFlashAdapter {
    pub fn new(pool: Address) -> Self {
        Self { pool }
    }
}

impl Default for HyperLendNativeFlashAdapter {
    fn default() -> Self {
        Self { pool: Address::zero() }
    }
}

#[async_trait]
impl FlashLoanAdapter for HyperLendNativeFlashAdapter {
    fn id(&self) -> &'static str {
        "hyperlend_native"
    }

    async fn quote_fee(&self, _asset: Address, amount: U256) -> Result<U256> {
        // HyperLend flash premium: 5 bps (0.05%)
        Ok(amount * 5 / 10000)
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoan(address,address[],uint256[],uint256[],address,bytes,uint16)
        // selector: 0xab9c4b5d
        let sel = [0xab, 0x9c, 0x4b, 0x5d];
        let params = encode(&[
            Token::Address(Address::zero()), // receiver
            Token::Array(vec![Token::Address(asset)]),
            Token::Array(vec![Token::Uint(amount)]),
            Token::Array(vec![Token::Uint(U256::zero())]), // mode 0: flash loan
            Token::Address(Address::zero()), // onBehalfOf
            Token::Bytes(inner_calldata.to_vec()),
            Token::Uint(U256::zero()), // referralCode
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
