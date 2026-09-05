//! Balancer V3 FlashLoanAdapter.
//! 03_ADAPTER_ARCHITECTURE.md, 06_CHAIN_BASE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;

pub struct BalancerV3FlashAdapter {
    pub vault: Address,
}

impl BalancerV3FlashAdapter {
    pub fn new(vault: Address) -> Self {
        Self { vault }
    }
}

impl Default for BalancerV3FlashAdapter {
    fn default() -> Self {
        Self {
            vault: Address::zero(),
        }
    }
}

#[async_trait]
impl FlashLoanAdapter for BalancerV3FlashAdapter {
    fn id(&self) -> &'static str {
        "balancer_v3"
    }

    async fn quote_fee(&self, _asset: Address, _amount: U256) -> Result<U256> {
        Ok(U256::zero())
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoan(address recipient, address[] tokens, uint256[] amounts, bytes userData)
        let sel = [0x5e, 0x08, 0x56, 0x55];
        let params = encode(&[
            Token::Address(Address::zero()),
            Token::Array(vec![Token::Address(asset)]),
            Token::Array(vec![Token::Uint(amount)]),
            Token::Bytes(inner_calldata.to_vec()),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
