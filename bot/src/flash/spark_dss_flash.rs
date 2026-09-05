//! Spark / Maker DSS FlashLoanAdapter for Ethereum mainnet.
//! 07_FLASH_LOANS_AND_DEX.md §7.1, §7.2

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;
use crate::shared::addresses::ethereum;

pub struct SparkDssFlashAdapter {
    pub flash_contract: Address,
}

impl SparkDssFlashAdapter {
    pub fn new(flash_contract: Address) -> Self {
        Self { flash_contract }
    }
}

impl Default for SparkDssFlashAdapter {
    fn default() -> Self {
        Self {
            // Maker / Spark DSS Flash canonical address on Ethereum mainnet (07 §7.1)
            flash_contract: ethereum::SPARK_DSS_FLASH.parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl FlashLoanAdapter for SparkDssFlashAdapter {
    fn id(&self) -> &'static str {
        "spark_dss_flash"
    }

    async fn quote_fee(&self, _asset: Address, _amount: U256) -> Result<U256> {
        // Spark DSS Flash has 0.00% fee
        Ok(U256::zero())
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoan(address receiver, address token, uint256 amount, bytes calldata data)
        // selector: 0x5cffe9de
        let sel = [0x5c, 0xff, 0xe9, 0xde];
        let params = encode(&[
            Token::Address(Address::zero()),
            Token::Address(asset),
            Token::Uint(amount),
            Token::Bytes(inner_calldata.to_vec()),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
