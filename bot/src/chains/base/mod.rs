//! Base (Chain ID 8453) adapter module.
//! 06_CHAIN_BASE.md

pub mod aave_v3;
pub mod morpho_blue;
pub mod swaps;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
    use crate::flash::FlashLoanAdapter;
    use crate::swap::SwapVenueAdapter;
    use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};
    use ethers::types::{Address, Bytes, U256};

    #[tokio::test]
    async fn test_base_aave_v3_adapter() {
        let adapter = aave_v3::BaseAaveV3Adapter::default();
        assert_eq!(adapter.id(), "aave_v3");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(1_000_000u64),
            collateral_amount: U256::from(2_000_000u64),
            health_factor: 1.02,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: ethers::types::TxHash::zero(),
            last_update_block: 100,
        };

        adapter.set_positions(vec![pos.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let at_risk = adapter.positions_below_hf(1.03).await;
        assert_eq!(at_risk.len(), 1);
        assert_eq!(at_risk[0].borrower, pos.borrower);

        let safe = adapter.positions_below_hf(1.01).await;
        assert_eq!(safe.len(), 0);

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::from(vec![1, 2, 3]),
            expected_out: U256::from(990_000u64),
        };
        let cd = adapter.build_liquidation_calldata(&pos, route, U256::from(1000)).await;
        assert!(cd.is_ok());
        assert!(!cd.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_base_morpho_blue_adapter() {
        let adapter = morpho_blue::BaseMorphoBlueAdapter::default();
        assert_eq!(adapter.id(), "morpho_blue");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(500_000u64),
            collateral_amount: U256::from(1_000_000u64),
            health_factor: 0.98,
            protocol: LendingProtocol::Morpho,
            morpho_market_params: Bytes::from(vec![0xAA; 32]),
            morpho_market_id: ethers::types::TxHash::zero(),
            last_update_block: 100,
        };

        adapter.set_positions(vec![pos.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let at_risk = adapter.positions_below_hf(1.00).await;
        assert_eq!(at_risk.len(), 1);
        assert_eq!(at_risk[0].protocol, LendingProtocol::Morpho);

        let route = SwapRoute {
            venue_id: "aerodrome".into(),
            path: Bytes::new(),
            expected_out: U256::from(490_000u64),
        };
        let cd = adapter.build_liquidation_calldata(&pos, route, U256::from(500)).await;
        assert!(cd.is_ok());
    }

    #[tokio::test]
    async fn test_base_swap_adapters() {
        let aero = swaps::AerodromeAdapter::default();
        assert_eq!(aero.id(), "aerodrome");
        let q = aero.quote(Address::zero(), Address::zero(), U256::from(1000)).await.unwrap();
        assert_eq!(q, U256::from(980));
        let call = aero.build_swap_call(Address::zero(), Address::zero(), U256::from(1000), U256::from(950));
        assert!(!call.is_empty());

        let uni = swaps::BaseUniswapV3Adapter::default();
        assert_eq!(uni.id(), "uniswap_v3");
        let q_uni = uni.quote(Address::zero(), Address::zero(), U256::from(1000)).await.unwrap();
        assert_eq!(q_uni, U256::from(982));
        let call_uni = uni.build_swap_call(Address::zero(), Address::zero(), U256::from(1000), U256::from(950));
        assert!(!call_uni.is_empty());
    }

    #[tokio::test]
    async fn test_base_flash_adapters() {
        use crate::flash::{aave_v3::AaveV3FlashAdapter, balancer_v2::BalancerV2FlashAdapter, morpho_blue::MorphoBlueFlashAdapter};

        let bal = BalancerV2FlashAdapter::default();
        assert_eq!(bal.id(), "balancer_v2");
        assert_eq!(bal.quote_fee(Address::zero(), U256::from(10000)).await.unwrap(), U256::zero());

        let morpho = MorphoBlueFlashAdapter::default();
        assert_eq!(morpho.id(), "morpho_blue");
        assert_eq!(morpho.quote_fee(Address::zero(), U256::from(10000)).await.unwrap(), U256::zero());

        let aave = AaveV3FlashAdapter::default();
        assert_eq!(aave.id(), "aave_v3");
        assert_eq!(aave.quote_fee(Address::zero(), U256::from(10000)).await.unwrap(), U256::from(5));
    }
}
