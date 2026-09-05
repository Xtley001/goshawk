//! Aave V3 FlashLoanAdapter.
//! 03_ADAPTER_ARCHITECTURE.md, 06_CHAIN_BASE.md, 07_CHAIN_AAVE_V3_STANDARD.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;

pub struct AaveV3FlashAdapter {
    pub pool: Address,
}

impl AaveV3FlashAdapter {
    pub fn new(pool: Address) -> Self {
        Self { pool }
    }
}

impl Default for AaveV3FlashAdapter {
    fn default() -> Self {
        Self { pool: Address::zero() }
    }
}

#[async_trait]
impl FlashLoanAdapter for AaveV3FlashAdapter {
    fn id(&self) -> &'static str {
        "aave_v3"
    }

    async fn quote_fee(&self, _asset: Address, amount: U256) -> Result<U256> {
        // Standard Aave V3 flash loan fee: 5 bps (0.05%)
        Ok(amount * 5 / 10000)
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoanSimple(address receiverAddress, address asset, uint256 amount, bytes params, uint16 referralCode)
        // selector: 0x42966c68
        let sel = [0x42, 0x96, 0x6c, 0x68];
        let params = encode(&[
            Token::Address(Address::zero()), // receiver
            Token::Address(asset),
            Token::Uint(amount),
            Token::Bytes(inner_calldata.to_vec()),
            Token::Uint(U256::zero()), // referralCode 0
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
