//! Flash loan adapter trait and registry.
//! 03_ADAPTER_ARCHITECTURE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::collections::HashMap;

pub mod morpho_blue;
pub mod balancer_v2;
pub mod spark_dss_flash;
pub mod aave_v3;

#[async_trait]
pub trait FlashLoanAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    async fn quote_fee(&self, asset: Address, amount: U256) -> Result<U256>;
    fn build_flash_call(&self, asset: Address, amount: U256, inner_calldata: Bytes) -> Bytes;
}

#[derive(Default)]
pub struct FlashLoanRegistry {
    adapters: HashMap<String, Box<dyn FlashLoanAdapter>>,
}

impl FlashLoanRegistry {
    pub fn new() -> Self {
        Self { adapters: HashMap::new() }
    }

    pub fn register(&mut self, adapter: Box<dyn FlashLoanAdapter>) {
        self.adapters.insert(adapter.id().to_string(), adapter);
    }

    pub fn get(&self, id: &str) -> Option<&(dyn FlashLoanAdapter + 'static)> {
        self.adapters.get(id).map(|b| b.as_ref())
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flash_loan_registry_all_providers() {
        let mut reg = FlashLoanRegistry::new();
        reg.register(Box::new(morpho_blue::MorphoBlueFlashAdapter::default()));
        reg.register(Box::new(balancer_v2::BalancerV2FlashAdapter::default()));
        reg.register(Box::new(spark_dss_flash::SparkDssFlashAdapter::default()));
        reg.register(Box::new(aave_v3::AaveV3FlashAdapter::default()));

        assert_eq!(reg.len(), 4);

        // Exactly the four providers from 07_FLASH_LOANS_AND_DEX.md §7.2
        let expected_providers = [
            ("morpho_blue", U256::zero()),
            ("balancer_v2", U256::zero()),
            ("spark_dss_flash", U256::zero()),
            ("aave_v3", U256::from(50u64)), // 5 bps on 100,000 = 50
        ];

        let amount = U256::from(100_000u64);
        for (id, expected_fee) in expected_providers {
            let adapter = reg.get(id).unwrap_or_else(|| panic!("Flash adapter {} missing", id));
            assert_eq!(adapter.id(), id);
            let fee = adapter.quote_fee(Address::zero(), amount).await.unwrap();
            assert_eq!(fee, expected_fee, "Fee mismatch for provider {}", id);
            let dummy_inner = Bytes::from(vec![1, 2, 3, 4]);
            let call = adapter.build_flash_call(Address::zero(), amount, dummy_inner);
            assert!(!call.is_empty(), "Calldata must not be empty for {}", id);
        }
    }
}

