//! Balancer SwapVenueAdapter (Gnosis, Ethereum, Base).
//! 07_CHAIN_AAVE_V3_STANDARD.md, 08_CHAIN_ETHEREUM.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

use crate::shared::addresses::ethereum;
use crate::swap::SwapVenueAdapter;

pub struct BalancerSwapAdapter {
    pub vault: Address,
}

impl BalancerSwapAdapter {
    pub fn new(vault: Address) -> Self {
        Self { vault }
    }
}

impl Default for BalancerSwapAdapter {
    fn default() -> Self {
        Self {
            // Balancer V2 Vault canonical address on Ethereum mainnet (07 §7.1)
            vault: ethereum::BALANCER_VAULT.parse().unwrap_or(Address::zero()),
        }
    }
}

#[async_trait]
impl SwapVenueAdapter for BalancerSwapAdapter {
    fn id(&self) -> &'static str {
        "balancer"
    }

    async fn quote(&self, _token_in: Address, _token_out: Address, amount_in: U256) -> Result<U256> {
        // Balancer weighted pool fee ~25-30 bps
        Ok(amount_in * 997 / 1000)
    }

    fn build_swap_call(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_out: U256,
    ) -> Bytes {
        // swap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool),uint256,uint256)
        // selector: 0x52bbbe29
        let sel = [0x52, 0xbb, 0xbe, 0x29];
        let single_swap = Token::Tuple(vec![
            Token::FixedBytes(vec![0u8; 32]), // poolId (resolved by executor)
            Token::Uint(U256::zero()),        // GIVEN_IN
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Uint(amount_in),
            Token::Bytes(vec![]),
        ]);
        let funds = Token::Tuple(vec![
            Token::Address(Address::zero()), // sender
            Token::Bool(false),
            Token::Address(Address::zero()), // recipient
            Token::Bool(false),
        ]);
        let params = encode(&[
            single_swap,
            funds,
            Token::Uint(min_out),
            Token::Uint(U256::from(u64::MAX)),
        ]);
        Bytes::from([sel.as_slice(), params.as_slice()].concat())
    }
}
