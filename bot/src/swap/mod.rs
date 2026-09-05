//! Swap venue adapter trait and registry.
//! 03_ADAPTER_ARCHITECTURE.md

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::{Address, Bytes, U256};
use std::collections::HashMap;

pub mod uniswap_v3;
pub mod curve;
pub mod balancer;

#[async_trait]
pub trait SwapVenueAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    async fn quote(&self, token_in: Address, token_out: Address, amount_in: U256) -> Result<U256>;
    fn build_swap_call(&self, token_in: Address, token_out: Address, amount_in: U256, min_out: U256) -> Bytes;
}

#[derive(Default)]
pub struct SwapVenueRegistry {
    venues: HashMap<String, Box<dyn SwapVenueAdapter>>,
}

impl SwapVenueRegistry {
    pub fn new() -> Self {
        Self { venues: HashMap::new() }
    }

    pub fn register(&mut self, venue: Box<dyn SwapVenueAdapter>) {
        self.venues.insert(venue.id().to_string(), venue);
    }

    pub fn get(&self, id: &str) -> Option<&(dyn SwapVenueAdapter + 'static)> {
        self.venues.get(id).map(|b| b.as_ref())
    }

    pub fn len(&self) -> usize {
        self.venues.len()
    }

    pub fn is_empty(&self) -> bool {
        self.venues.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swap_venue_registry_all_venues() {
        let mut reg = SwapVenueRegistry::new();
        reg.register(Box::new(uniswap_v3::GenericUniswapV3Adapter::default()));
        reg.register(Box::new(curve::CurveAdapter::default()));
        reg.register(Box::new(balancer::BalancerSwapAdapter::default()));

        assert_eq!(reg.len(), 3);

        let venues = ["uniswap_v3", "curve", "balancer"];
        for v in venues {
            let adapter = reg.get(v).unwrap_or_else(|| panic!("Venue {} missing", v));
            assert_eq!(adapter.id(), v);
            let q = adapter.quote(Address::zero(), Address::zero(), U256::from(10_000)).await.unwrap();
            assert!(q > U256::zero());
            let call = adapter.build_swap_call(Address::zero(), Address::zero(), U256::from(1000), U256::from(950));
            assert!(!call.is_empty());
        }
    }
}
