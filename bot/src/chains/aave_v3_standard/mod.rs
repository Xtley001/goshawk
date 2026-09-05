//! Standard Aave V3 adapter module (Arbitrum, Optimism, Polygon, Avalanche, BNB, Gnosis).
//! 07_CHAIN_AAVE_V3_STANDARD.md

pub mod aave_v3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
    use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};
    use ethers::types::{Address, Bytes, TxHash, U256};

    #[tokio::test]
    async fn test_standard_aave_v3_adapter() {
        let adapter = aave_v3::StandardAaveV3Adapter::new(
            "arbitrum",
            Address::random(),
            Address::random(),
        );
        assert_eq!(adapter.id(), "aave_v3");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 1.01,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        adapter.set_positions(vec![pos.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let at_risk = adapter.positions_below_hf(1.03).await;
        assert_eq!(at_risk.len(), 1);

        let safe = adapter.positions_below_hf(1.00).await;
        assert_eq!(safe.len(), 0);

        let route = SwapRoute {
            venue_id: "curve".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };
        let cd = adapter.build_liquidation_calldata(&pos, route, U256::from(500)).await;
        assert!(cd.is_ok());
        assert!(!cd.unwrap().is_empty());
    }
}
