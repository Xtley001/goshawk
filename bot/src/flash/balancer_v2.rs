//! Balancer V2 FlashLoanAdapter.
//! 03_ADAPTER_ARCHITECTURE.md, 06_CHAIN_BASE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;

pub struct BalancerV2FlashAdapter {
    pub vault: Address,
}

impl BalancerV2FlashAdapter {
    pub fn new(vault: Address) -> Self {
        Self { vault }
    }
}

impl Default for BalancerV2FlashAdapter {
    fn default() -> Self {
        Self {
            vault: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl FlashLoanAdapter for BalancerV2FlashAdapter {
    fn id(&self) -> &'static str {
        "balancer_v2"
    }

    async fn quote_fee(&self, _asset: Address, _amount: U256) -> Result<U256> {
        // Balancer V2 has 0 fee for flash loans
        Ok(U256::zero())
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoan(address recipient, address[] tokens, uint256[] amounts, bytes userData)
        // selector: 0x5e085655
        let sel = [0x5e, 0x08, 0x56, 0x55];
        let params = encode(&[
            Token::Address(Address::zero()), // recipient filled by caller/executor
            Token::Array(vec![Token::Address(asset)]),
            Token::Array(vec![Token::Uint(amount)]),
            Token::Bytes(inner_calldata.to_vec()),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
