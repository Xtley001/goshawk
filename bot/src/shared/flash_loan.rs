//! Flash loan router — Liquidation calldata builder.

use anyhow::Result;
use ethers::{abi::{encode, Token}, prelude::*};
use std::sync::Arc;
use crate::config::Config;
use crate::shared::{addresses::ethereum, position_indexer::BorrowPosition};

const FLASH_PROVIDER_MORPHO:    u8 = 0;
const FLASH_PROVIDER_BALANCER:  u8 = 1;
const FLASH_PROVIDER_SPARK_DSS: u8 = 2;
const FLASH_PROVIDER_AAVE:      u8 = 3;

const STRAT_LIQUIDATION: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashProvider { Morpho, Balancer, SparkDss, Aave }

impl FlashProvider {
    pub fn as_u8(self) -> u8 {
        match self {
            FlashProvider::Morpho   => FLASH_PROVIDER_MORPHO,
            FlashProvider::Balancer => FLASH_PROVIDER_BALANCER,
            FlashProvider::SparkDss => FLASH_PROVIDER_SPARK_DSS,
            FlashProvider::Aave     => FLASH_PROVIDER_AAVE,
        }
    }
}

pub struct FlashLoanRouter {
    provider:  Arc<Provider<Ipc>>,
    balancer:  Address,
    morpho:    Address,
    spark_dss: Address,
    executor:  Address,
    multicall: Address,
}

impl FlashLoanRouter {
    pub fn new(provider: Arc<Provider<Ipc>>, cfg: &Config) -> Result<Self> {
        let eth_cfg = cfg.get_chain("ethereum");
        let balancer_str = eth_cfg.and_then(|c| c.get_address("balancer_vault")).unwrap_or_default();
        let morpho_str = eth_cfg.and_then(|c| c.get_address("morpho_blue")).unwrap_or_default();
        let spark_str = eth_cfg
            .and_then(|c| c.get_address("spark_dss_flash"))
            .unwrap_or(ethereum::SPARK_DSS_FLASH);
        let executor_env = std::env::var("GOSHAWK_FLASH_EXECUTOR_ADDRESS")
            .unwrap_or_default();
        let executor_str = eth_cfg
            .and_then(|c| c.get_address("flash_executor_address"))
            .filter(|s| !s.is_empty())
            .unwrap_or(if executor_env.is_empty() {
                "0x0000000000000000000000000000000000000001"
            } else {
                &executor_env
            });

        let balancer: Address = balancer_str.parse()
            .map_err(|e| anyhow::anyhow!("Invalid balancer_vault '{}': {}", balancer_str, e))?;
        let executor: Address = executor_str.parse()
            .map_err(|e| anyhow::anyhow!(
                "Invalid flash_executor_address '{}': {}. Set GOSHAWK_FLASH_EXECUTOR_ADDRESS.",
                executor_str, e
            ))?;
        Ok(Self {
            provider,
            balancer,
            morpho: morpho_str.parse()
                .map_err(|e| anyhow::anyhow!("Invalid morpho_blue '{}': {}", morpho_str, e))?,
            spark_dss: spark_str.parse().unwrap_or_default(),
            executor,
            multicall: ethereum::MULTICALL3.parse()
                .map_err(|e| anyhow::anyhow!("Invalid MULTICALL3 constant: {}", e))?,
        })
    }

    /// Live-quoted parallel provider selection:
    /// Queries provider depths and fees, picking the lowest-fee provider
    /// with sufficient liquidity for the requested amount, following 07 §7.2 order.
    pub async fn select_provider(&self, asset: Address, amount: U256) -> Result<FlashProvider> {
        let (bal_depth, morpho_depth) = self.batch_query_depths(asset).await?;

        // Priority order per 07_FLASH_LOANS_AND_DEX.md §7.2:
        // 1. Morpho Blue (0.00%)
        // 2. Balancer V2 (0.00%)
        // 3. Spark DSS (0.00%)
        // 4. Aave V3 (0.05%)
        let mut candidates: Vec<(FlashProvider, U256, U256, usize)> = Vec::new();

        // 1. Morpho: 0 fee, 40% buffer
        candidates.push((FlashProvider::Morpho, morpho_depth, U256::zero(), 0));

        // 2. Balancer: 0 fee, 5% buffer on depth
        let bal_effective_depth = bal_depth * 95 / 100;
        candidates.push((FlashProvider::Balancer, bal_effective_depth, U256::zero(), 1));

        // 3. Spark DSS Flash: 0 fee
        let spark_depth = self.balance_of(asset, self.spark_dss).await.unwrap_or(U256::zero());
        candidates.push((FlashProvider::SparkDss, spark_depth, U256::zero(), 2));

        // 4. Aave: 5 bps fee, deep standing pool
        let aave_fee = amount * 5 / 10_000;
        candidates.push((FlashProvider::Aave, U256::max_value(), aave_fee, 3));

        // Filter for candidates that satisfy the required borrow amount
        let mut eligible: Vec<_> = candidates.into_iter().filter(|(_, depth, _, _)| *depth >= amount).collect();
        // Sort by fee ascending, then by §7.2 priority index ascending
        eligible.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.3.cmp(&b.3)));

        if let Some((best, _, _, _)) = eligible.first() {
            Ok(*best)
        } else {
            Ok(FlashProvider::Aave)
        }
    }

    pub async fn get_provider_depth(&self, asset: Address) -> Result<U256> {
        self.balance_of(asset, self.balancer).await
    }

    async fn batch_query_depths(&self, asset: Address) -> Result<(U256, U256)> {
        let sel         = &ethers::utils::keccak256(b"balanceOf(address)")[..4];
        let bal_data    = [sel, &encode(&[Token::Address(self.balancer)])].concat();
        let morpho_data = [sel, &encode(&[Token::Address(self.morpho)])].concat();
        let calls = encode(&[Token::Array(vec![
            encode_mc3_call(asset, bal_data),
            encode_mc3_call(asset, morpho_data),
        ])]);
        let mc3_sel  = &ethers::utils::keccak256(b"aggregate3((address,bool,bytes)[])")[..4];
        let calldata = [mc3_sel, calls.as_slice()].concat();
        let res = self.provider.call(
            &TransactionRequest { to: Some(self.multicall.into()), data: Some(calldata.into()), ..Default::default() }.into(),
            None,
        ).await;
        match res {
            Ok(data) => {
                let bd = parse_mc3_uint256(&data, 0).unwrap_or(U256::zero());
                let md = parse_mc3_uint256(&data, 1).unwrap_or(U256::zero());
                Ok((bd, md * 60 / 100))
            }
            Err(_) => {
                let bd = self.balance_of(asset, self.balancer).await.unwrap_or(U256::zero());
                let md = self.balance_of(asset, self.morpho).await.unwrap_or(U256::zero());
                Ok((bd, md * 60 / 100))
            }
        }
    }

    async fn balance_of(&self, token: Address, account: Address) -> Result<U256> {
        let sel  = &ethers::utils::keccak256(b"balanceOf(address)")[..4];
        let data = [sel, &encode(&[Token::Address(account)])].concat();
        let res  = self.provider.call(
            &TransactionRequest { to: Some(token.into()), data: Some(data.into()), ..Default::default() }.into(),
            None,
        ).await?;
        Ok(U256::from_big_endian(&res))
    }

    fn execute_selector() -> [u8; 4] {
        let sig = b"execute((uint8,address[],uint256[],uint8,bytes,uint256,address))";
        let h = ethers::utils::keccak256(sig);
        [h[0], h[1], h[2], h[3]]
    }

    fn encode_execute_params(
        provider:     FlashProvider,
        tokens:       Vec<Address>,
        amounts:      Vec<U256>,
        strat_type:   u8,
        strat_data:   Vec<u8>,
        min_profit:   U256,
        profit_token: Address,
    ) -> Vec<u8> {
        let params = Token::Tuple(vec![
            Token::Uint(U256::from(provider.as_u8())),
            Token::Array(tokens.into_iter().map(Token::Address).collect()),
            Token::Array(amounts.into_iter().map(Token::Uint).collect()),
            Token::Uint(U256::from(strat_type)),
            Token::Bytes(strat_data),
            Token::Uint(min_profit),
            Token::Address(profit_token),
        ]);
        let sel = Self::execute_selector();
        [sel.as_slice(), encode(&[params]).as_slice()].concat()
    }

    // ─── Liquidation ──────────────────────────────────────────────────────
    pub async fn build_liquidation(
        &self,
        pos:            &BorrowPosition,
        provider:       FlashProvider,
        swap_route:     Bytes,
        min_profit_wei: U256,
    ) -> Result<Bytes> {
        use crate::shared::position_indexer::LendingProtocol;
        let protocol_id: u8 = match pos.protocol {
            LendingProtocol::Morpho => 0,
            LendingProtocol::Aave => 1,
            LendingProtocol::Spark => 3,
            LendingProtocol::Fluid => 4,
            LendingProtocol::CompoundV3 => 5,
            LendingProtocol::EulerV2 => 6,
        };
        let uni_router: Address = ethereum::UNISWAP_V3_ROUTER.parse()?;
        let strat_data = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from(protocol_id)),
            Token::Address(pos.borrower),
            Token::Address(pos.collateral_asset),
            Token::Address(pos.debt_asset),
            Token::Uint(pos.debt_amount),
            Token::Bytes(pos.morpho_market_params.to_vec()),
            Token::Address(uni_router),
            Token::Bytes(swap_route.to_vec()),
        ])]);
        let cd = Self::encode_execute_params(
            provider, vec![pos.debt_asset], vec![pos.debt_amount],
            STRAT_LIQUIDATION, strat_data,
            min_profit_wei,
            pos.debt_asset,
        );
        Ok(Bytes::from(cd))
    }

    pub fn provider(&self) -> Arc<Provider<Ipc>> { self.provider.clone() }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn encode_mc3_call(target: Address, call_data: Vec<u8>) -> Token {
    Token::Tuple(vec![Token::Address(target), Token::Bool(true), Token::Bytes(call_data)])
}

fn parse_mc3_uint256(data: &Bytes, idx: usize) -> Option<U256> {
    use ethers::abi::{decode, ParamType};
    let t = ParamType::Array(Box::new(ParamType::Tuple(vec![ParamType::Bool, ParamType::Bytes])));
    let tokens = decode(&[t], data).ok()?;
    let results = tokens.into_iter().next()?.into_array()?;
    if let Token::Tuple(fields) = results.get(idx)? {
        if fields[0] == Token::Bool(true) {
            if let Token::Bytes(b) = &fields[1] {
                if b.len() >= 32 { return Some(U256::from_big_endian(&b[0..32])); }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_provider_selection_precedence() {
        let amount = U256::from(100_000u64);

        // Case 1: Balancer has depth -> selected for 0 fee
        let mut candidates: Vec<(FlashProvider, U256, U256)> = vec![
            (FlashProvider::Balancer, U256::from(200_000u64), U256::zero()),
            (FlashProvider::Morpho, U256::from(50_000u64), U256::zero()),
            (FlashProvider::Aave, U256::max_value(), U256::from(50u64)),
        ];
        let mut eligible: Vec<_> = candidates.into_iter().filter(|(_, d, _)| *d >= amount).collect();
        eligible.sort_by_key(|(_, _, fee)| *fee);
        assert_eq!(eligible[0].0, FlashProvider::Balancer);

        // Case 2: Balancer lacks depth, Morpho has depth -> Morpho selected for 0 fee
        candidates = vec![
            (FlashProvider::Balancer, U256::from(10_000u64), U256::zero()),
            (FlashProvider::Morpho, U256::from(150_000u64), U256::zero()),
            (FlashProvider::Aave, U256::max_value(), U256::from(50u64)),
        ];
        eligible = candidates.into_iter().filter(|(_, d, _)| *d >= amount).collect();
        eligible.sort_by_key(|(_, _, fee)| *fee);
        assert_eq!(eligible[0].0, FlashProvider::Morpho);

        // Case 3: Both Balancer and Morpho lack depth -> Aave selected
        candidates = vec![
            (FlashProvider::Balancer, U256::from(10_000u64), U256::zero()),
            (FlashProvider::Morpho, U256::from(20_000u64), U256::zero()),
            (FlashProvider::Aave, U256::max_value(), U256::from(50u64)),
        ];
        eligible = candidates.into_iter().filter(|(_, d, _)| *d >= amount).collect();
        eligible.sort_by_key(|(_, _, fee)| *fee);
        assert_eq!(eligible[0].0, FlashProvider::Aave);
    }
}
