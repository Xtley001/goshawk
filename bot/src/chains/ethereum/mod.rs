//! Ethereum (Chain ID 1) adapter module.
//! 08_CHAIN_ETHEREUM.md

pub mod aave_v3;
pub mod spark;
pub mod morpho_blue;
pub mod fluid;
pub mod compound_v3;
pub mod euler_v2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::{LendingMarketAdapter, OracleKind, SwapRoute};
    use crate::shared::position_indexer::{BorrowPosition, LendingProtocol};
    use ethers::types::{Address, Bytes, TxHash, U256};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_spark_adapter() {
        let adapter = spark::SparkAdapter::new(Address::random(), Address::random());
        assert_eq!(adapter.id(), "spark");
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

        let route = SwapRoute {
            venue_id: "curve".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };
        let cd = adapter.build_liquidation_calldata(&pos, route, U256::from(500)).await;
        assert!(cd.is_ok());
    }

    #[tokio::test]
    async fn test_ethereum_aave_v3_svr_exclusion() {
        let adapter = aave_v3::EthereumAaveV3Adapter::new(Address::random(), Address::random());
        assert_eq!(adapter.id(), "aave_v3_ethereum");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        // tBTC address on mainnet
        let tbtc = Address::from_str("0x18084fbA666a33d37592fA2633fD49a74DD93a88").unwrap();
        // AAVE token on mainnet
        let aave = Address::from_str("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9").unwrap();
        let regular_asset = Address::random();
        let usdc = Address::random();

        assert!(adapter.is_svr_excluded(tbtc, usdc));
        assert!(adapter.is_svr_excluded(regular_asset, aave));
        assert!(!adapter.is_svr_excluded(regular_asset, usdc));

        let pos_svr = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: tbtc, // SVR asset!
            debt_asset: usdc,
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 1.01,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        let pos_ok = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: regular_asset,
            debt_asset: usdc,
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 1.01,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        adapter.set_positions(vec![pos_svr.clone(), pos_ok.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // The SVR-gated position MUST be filtered out completely
        let candidates = adapter.positions_below_hf(1.03).await;
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].borrower, pos_ok.borrower);

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        // Building calldata for SVR position must fail
        assert!(adapter.build_liquidation_calldata(&pos_svr, route.clone(), U256::from(100)).await.is_err());
        // Building calldata for regular position must succeed
        assert!(adapter.build_liquidation_calldata(&pos_ok, route, U256::from(100)).await.is_ok());
    }

    #[tokio::test]
    async fn test_morpho_blue_rejects_zero_market_params() {
        let adapter = morpho_blue::MorphoBlueAdapter::default();
        assert_eq!(adapter.id(), "morpho_blue");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);

        // Position with zero / empty market params
        let pos_zero = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 0.95,
            protocol: LendingProtocol::Morpho,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        let res = adapter.build_liquidation_calldata(&pos_zero, route.clone(), U256::from(100)).await;
        assert!(res.is_err(), "Morpho Blue MUST reject zero / unresolved market params");
        assert!(res.unwrap_err().to_string().contains("unresolved or zero MarketParams"));

        // Position with all-zero market params bytes
        let pos_all_zero_bytes = BorrowPosition {
            morpho_market_params: Bytes::from(vec![0u8; 32]),
            morpho_market_id: TxHash::from_str("0x8793cf302b8ffd655ab97bd1c695dbd967807e8367a65cb2f4edaf1380ba1bda").unwrap(),
            ..pos_zero.clone()
        };
        let res2 = adapter.build_liquidation_calldata(&pos_all_zero_bytes, route, U256::from(100)).await;
        assert!(res2.is_err(), "Morpho Blue MUST reject all-zero market params");
    }

    #[tokio::test]
    async fn test_morpho_blue_valid_liquidation() {
        let adapter = morpho_blue::MorphoBlueAdapter::default();

        let valid_market_id = TxHash::from_str("0x8793cf302b8ffd655ab97bd1c695dbd967807e8367a65cb2f4edaf1380ba1bda").unwrap();
        let valid_params = Bytes::from(vec![1u8; 32]);

        let pos_valid = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 0.92,
            protocol: LendingProtocol::Morpho,
            morpho_market_params: valid_params,
            morpho_market_id: valid_market_id,
            last_update_block: 200,
        };

        adapter.set_positions(vec![pos_valid.clone()]);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let at_risk = adapter.positions_below_hf(0.95).await;
        assert_eq!(at_risk.len(), 1);
        assert_eq!(at_risk[0].borrower, pos_valid.borrower);

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        let calldata = adapter.build_liquidation_calldata(&pos_valid, route, U256::from(100)).await;
        assert!(calldata.is_ok(), "Morpho Blue with valid MarketParams MUST succeed");
        assert!(!calldata.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fluid_adapter_inert_state_and_liquidation() {
        let adapter = fluid::FluidAdapter::default();
        assert_eq!(adapter.id(), "fluid");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);
        assert!(!adapter.is_enabled());

        // Inert state: positions_below_hf must return empty vec
        let at_risk = adapter.positions_below_hf(1.5).await;
        assert!(at_risk.is_empty());

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 0.85,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        // Calldata building should fail when disabled
        let res = adapter.build_liquidation_calldata(&pos, route.clone(), U256::from(100)).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("FLUID_LIQUIDITY_LAYER"));

        // Enabled adapter with configured address
        let enabled_adapter = fluid::FluidAdapter::new(Address::random());
        assert!(enabled_adapter.is_enabled());
        let calldata = enabled_adapter.build_liquidation_calldata(&pos, route, U256::from(100)).await;
        assert!(calldata.is_ok());
        assert!(!calldata.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_compound_v3_adapter_inert_state_and_liquidation() {
        let adapter = compound_v3::CompoundV3Adapter::default();
        assert_eq!(adapter.id(), "compound_v3");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);
        assert!(!adapter.is_enabled());

        // Inert state: positions_below_hf must return empty vec
        let at_risk = adapter.positions_below_hf(1.5).await;
        assert!(at_risk.is_empty());

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 0.80,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        // Calldata building should fail when disabled
        let res = adapter.build_liquidation_calldata(&pos, route.clone(), U256::from(100)).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("COMPOUND_V3_COMET"));

        // Enabled adapter with configured address
        let enabled_adapter = compound_v3::CompoundV3Adapter::new(Address::random());
        assert!(enabled_adapter.is_enabled());
        let calldata = enabled_adapter.build_liquidation_calldata(&pos, route, U256::from(100)).await;
        assert!(calldata.is_ok());
        assert!(!calldata.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_euler_v2_adapter_inert_state_and_liquidation() {
        let adapter = euler_v2::EulerV2Adapter::default();
        assert_eq!(adapter.id(), "euler_v2");
        assert_eq!(adapter.oracle_kind(), OracleKind::Chainlink);
        assert!(!adapter.is_enabled());

        // Inert state: positions_below_hf must return empty vec
        let at_risk = adapter.positions_below_hf(1.5).await;
        assert!(at_risk.is_empty());

        let pos = BorrowPosition {
            borrower: Address::random(),
            collateral_asset: Address::random(),
            debt_asset: Address::random(),
            debt_amount: U256::from(50_000u64),
            collateral_amount: U256::from(100_000u64),
            health_factor: 0.90,
            protocol: LendingProtocol::Aave,
            morpho_market_params: Bytes::new(),
            morpho_market_id: TxHash::zero(),
            last_update_block: 200,
        };

        let route = SwapRoute {
            venue_id: "uniswap_v3".into(),
            path: Bytes::new(),
            expected_out: U256::from(49_000u64),
        };

        // Calldata building should fail when disabled
        let res = adapter.build_liquidation_calldata(&pos, route.clone(), U256::from(100)).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("EULER_V2_VAULT_CONTROLLER"));

        // Enabled adapter with configured address
        let enabled_adapter = euler_v2::EulerV2Adapter::new(Address::random());
        assert!(enabled_adapter.is_enabled());
        let calldata = enabled_adapter.build_liquidation_calldata(&pos, route, U256::from(100)).await;
        assert!(calldata.is_ok());
        assert!(!calldata.unwrap().is_empty());
    }
}

