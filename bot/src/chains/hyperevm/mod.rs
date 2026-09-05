//! HyperEVM (Chain ID 999) adapter module.
//! 05_CHAIN_HYPEREVM.md

pub mod hyperlend;
pub mod hypercore_client;
pub mod hyperswap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
    use crate::flash::FlashLoanAdapter;
    use crate::swap::SwapVenueAdapter;
    use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};
    use ethers::types::{Address, Bytes, TxHash, U256};

    #[tokio::test]
    async fn test_hyperlend_adapter_oracle_and_hf() {
        let adapter = hyperlend::HyperLendAdapter::default();
        assert_eq!(adapter.id(), "hyperlend");
        assert_eq!(adapter.oracle_kind(), OracleKind::HyperCorePrecompile);

        // Verify close factor logic (Audit Fix #7)
        assert_eq!(hyperlend::HyperLendAdapter::close_factor_bps(0.92), 10_000); // 100% when HF < 0.95
        assert_eq!(hyperlend::HyperLendAdapter::close_factor_bps(1.02), 5_000);  // 50% otherwise

        let total_debt = U256::from(100_000u64);
        assert_eq!(
            hyperlend::HyperLendAdapter::max_liquidatable_debt(total_debt, 0.90),
            total_debt
        );
        assert_eq!(
            hyperlend::HyperLendAdapter::max_liquidatable_debt(total_debt, 1.02),
            U256::from(50_000u64)
        );

        let pos_underwater = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: total_debt,
            collateral_amount: U256::from(200_000u64),
            health_factor: 0.98,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 100,
        };

        let pos_safe = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: total_debt,
            collateral_amount: U256::from(200_000u64),
            health_factor: 1.15,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 100,
        };

        adapter.set_positions(vec![pos_underwater.clone(), pos_safe.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Test staged threshold filtering: 1.10 presimulate, 1.05 submit, 1.00 liquidatable
        let presim = adapter.positions_below_hf(1.10).await;
        assert_eq!(presim.len(), 1);
        assert_eq!(presim[0].borrower, pos_underwater.borrower);

        let liq = adapter.positions_below_hf(1.00).await;
        assert_eq!(liq.len(), 1);

        let safe = adapter.positions_below_hf(0.95).await;
        assert_eq!(safe.len(), 0);

        let route = SwapRoute {
            venue_id: "hyperswap_v2".into(),
            path: Bytes::new(),
            expected_out: U256::from(95_000u64),
        };
        let cd = adapter.build_liquidation_calldata(&pos_underwater, route, U256::from(100)).await;
        assert!(cd.is_ok());
        assert!(!cd.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_hyperswap_adapter() {
        let swap = hyperswap::HyperSwapAdapter::default();
        assert_eq!(swap.id(), "hyperswap_v2");
        let q = swap.quote(Address::zero(), Address::zero(), U256::from(10_000)).await.unwrap();
        assert_eq!(q, U256::from(9_970)); // 30 bps fee

        let call = swap.build_swap_call(Address::zero(), Address::zero(), U256::from(1000), U256::from(950));
        assert!(!call.is_empty());
    }

    #[tokio::test]
    async fn test_hyperlend_native_flash() {
        use crate::flash::hyperlend_native::HyperLendNativeFlashAdapter;
        let flash = HyperLendNativeFlashAdapter::default();
        assert_eq!(flash.id(), "hyperlend_native");

        let fee = flash.quote_fee(Address::zero(), U256::from(100_000)).await.unwrap();
        assert_eq!(fee, U256::from(50)); // 5 bps

        let call = flash.build_flash_call(Address::zero(), U256::from(1000), Bytes::from(vec![1, 2, 3]));
        assert!(!call.is_empty());
    }
}
