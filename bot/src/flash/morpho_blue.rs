//! Morpho Blue FlashLoanAdapter.
//! 03_ADAPTER_ARCHITECTURE.md, 06_CHAIN_BASE.md, 08_CHAIN_ETHEREUM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::flash::FlashLoanAdapter;
use crate::shared::addresses::ethereum;

pub struct MorphoBlueFlashAdapter {
    pub morpho: Address,
}

impl MorphoBlueFlashAdapter {
    pub fn new(morpho: Address) -> Self {
        Self { morpho }
    }
}

impl Default for MorphoBlueFlashAdapter {
    fn default() -> Self {
        Self {
            // Morpho Blue canonical address on Ethereum mainnet (07 §7.1)
            morpho: ethereum::MORPHO_BLUE.parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl FlashLoanAdapter for MorphoBlueFlashAdapter {
    fn id(&self) -> &'static str {
        "morpho_blue"
    }

    async fn quote_fee(&self, _asset: Address, _amount: U256) -> Result<U256> {
        // Morpho Blue has 0 fee for flash loans
        Ok(U256::zero())
    }

    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes {
        // flashLoan(address token, uint256 assets, bytes data)
        // selector: 0xce21528f
        let sel = [0xce, 0x21, 0x52, 0x8f];
        let params = encode(&[
            Token::Address(asset),
            Token::Uint(amount),
            Token::Bytes(inner_calldata.to_vec()),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
