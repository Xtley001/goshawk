//! Flash loan adapter trait and registry.
//! 03_ADAPTER_ARCHITECTURE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::collections::HashMap;

pub mod balancer_v2;
pub mod balancer_v3;
pub mod aave_v3;
pub mod morpho_blue;
pub mod hyperlend_native;

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
