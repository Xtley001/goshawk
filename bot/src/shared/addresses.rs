//! Canonical Ethereum Mainnet (Chain ID 1) contract addresses.
//! Sourced per 05_PROTOCOLS_AND_ADDRESSES.md, 06_ORACLES.md, and 07_FLASH_LOANS_AND_DEX.md.
//! All unsourced gaps are explicitly marked TODO(GAP) per 12_RULES.md §12.5.

pub mod ethereum {
    // ── Flash loan providers (07_FLASH_LOANS_AND_DEX.md §7.1) ─────────────────
    /// Morpho Blue — 0.00% fee, verified Ethereum mainnet.
    pub const MORPHO_BLUE: &str = "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb";
    /// Balancer V2 Vault — 0.00% fee, verified Ethereum mainnet.
    pub const BALANCER_VAULT: &str = "0xBA12222222228d8Ba53be47888D16304ca09907c";
    /// Spark / Maker DSS Flash — 0.00% fee, verified Ethereum mainnet.
    pub const SPARK_DSS_FLASH: &str = "0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA";

    // ── Lending protocols (05_PROTOCOLS_AND_ADDRESSES.md) ────────────────────
    /// Aave V3 Pool Proxy (05 §5.1)
    pub const AAVE_V3_POOL_PROXY: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";
    /// Aave V3 Price Oracle (05 §5.1)
    pub const AAVE_V3_PRICE_ORACLE: &str = "0xb56c2F0B653B2e0b10C9b928C8580Ac5Df02C7C7";
    /// WBTC underlying on Aave V3 (05 §5.1)
    pub const AAVE_V3_WBTC_UNDERLYING: &str = "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f";
    /// aArbWBTC aToken (05 §5.1 — source labels this aArbWBTC, must verify on-chain)
    pub const AAVE_V3_AARBWTC: &str = "0x078f358208685046a11c85e8ad32895ded33a249";
    /// Aave V3 Variable debt token (05 §5.1)
    pub const AAVE_V3_VARIABLE_DEBT: &str = "0x92b42c66840c7ad907b4bf74879ff3ef7c529473";

    /// TODO(GAP): Spark SparkLend Pool proxy address is NOT SOURCED (05 §5.3, 12 §12.5)
    pub const SPARK_POOL_PROXY: &str = "";
    /// TODO(GAP): Fluid Liquidity Layer contract address is NOT SOURCED (05 §5.4, 12 §12.5)
    pub const FLUID_LIQUIDITY_LAYER: &str = "";
    /// TODO(GAP): Compound V3 Comet proxy address is NOT SOURCED (05 §5.5, 12 §12.5)
    pub const COMPOUND_V3_COMET: &str = "";
    /// TODO(GAP): Euler V2 VaultController address is NOT SOURCED (05 §5.6, 12 §12.5)
    pub const EULER_V2_VAULT_CONTROLLER: &str = "";

    // ── DEX infrastructure (07_FLASH_LOANS_AND_DEX.md §7.4) ───────────────────
    /// TODO(GAP): Uniswap V3 factory address on Ethereum mainnet is NOT SOURCED (07 §7.4, 12 §12.5)
    pub const UNISWAP_V3_FACTORY: &str = "";
    /// TODO(GAP): Uniswap V3 router address is NOT SOURCED (07 §7.4, 12 §12.5)
    pub const UNISWAP_V3_ROUTER: &str = "";
    /// TODO(GAP): Uniswap V3 QuoterV2 address is NOT SOURCED (07 §7.4, 12 §12.5)
    pub const UNISWAP_V3_QUOTER_V2: &str = "";
    /// TODO(GAP): Curve registry address on Ethereum mainnet is NOT SOURCED (07 §7.4, 12 §12.5)
    pub const CURVE_REGISTRY: &str = "";

    // ── Infrastructure ────────────────────────────────────────────────────────
    /// Multicall3 — canonical address on Ethereum mainnet.
    pub const MULTICALL3: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

    // ── Tokens (referenced across shared modules) ─────────────────────────────
    pub const WETH:   &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
    pub const USDC:   &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    pub const CBETH:  &str = "0xBe9895146f7AF43049ca1c1AE358B0541Ea49704";
    pub const USDBC:  &str = "";
    pub const WSTETH: &str = "0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0";
    pub const CBBTC:  &str = "";
    pub const DAI:    &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
    pub const AERODROME_FACTORY: &str = "";

    // ── Chainlink Price Feeds (06_ORACLES.md §6.1) ────────────────────────────
    /// ETH / USD — 8 decimals, 0.50% deviation, 3600s heartbeat
    pub const CHAINLINK_ETH_USD: &str = "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419";
    /// BTC / USD — 8 decimals, 0.50% deviation, 3600s heartbeat
    pub const CHAINLINK_BTC_USD: &str = "0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c";
    /// wstETH / ETH — 18 decimals, 0.50% deviation, 86400s heartbeat
    pub const CHAINLINK_WSTETH_ETH: &str = "0xAC1cD0C879E2773e254ae74000ba15441d1b950E";
    /// USDC / USD — 8 decimals, 0.25% deviation, 86400s heartbeat
    /// ⚠️ Flagged in 06 §6.1: anomalous run of eleven 9s, verify against live RPC before live use
    pub const CHAINLINK_USDC_USD: &str = "0x8fFf670264b163A7712399999999999999999999";
    /// cbBTC / USD — 8 decimals, 0.50% deviation, 3600s heartbeat
    pub const CHAINLINK_CBBTC_USD: &str = "0xF0a1740954e8035D012E4E6e1e695dAcFe7fe02f";

    // ── Gapped Feeds (06_ORACLES.md §6.2) ────────────────────────────────────
    /// TODO(GAP): Chainlink cbETH/ETH feed on Ethereum mainnet is NOT SOURCED (06 §6.2, 12 §12.5)
    pub const CHAINLINK_CBETH_ETH: &str = "";
    /// TODO(GAP): Chainlink USDE feed is NOT SOURCED (06 §6.2, 12 §12.5)
    pub const CHAINLINK_USDE_USD: &str = "";
    /// TODO(GAP): Chainlink USDS feed is NOT SOURCED (06 §6.2, 12 §12.5)
    pub const CHAINLINK_USDS_USD: &str = "";
    /// TODO(GAP): Chainlink senPYUSD feed is NOT SOURCED (06 §6.2, 12 §12.5)
    pub const CHAINLINK_SENPYUSD_USD: &str = "";
    /// TODO(GAP): Chainlink senRLUSD feed is NOT SOURCED (06 §6.2, 12 §12.5)
    pub const CHAINLINK_SENRLUSD_USD: &str = "";

    // ── Function selectors ────────────────────────────────────────────────────
    pub const SEL_TOKEN0: [u8; 4] = [0x0d, 0xfe, 0x16, 0x81]; // token0()
    pub const SEL_TOKEN1: [u8; 4] = [0xd2, 0x10, 0x77, 0x25]; // token1()
    pub const SEL_FEE:    [u8; 4] = [0xdd, 0xca, 0x3f, 0x43]; // fee()

    // ── Per-asset helpers ─────────────────────────────────────────────────────
    pub fn aave_liq_bonus(asset_lower_hex: &str) -> f64 {
        if asset_lower_hex.contains("c02aaa") { return 0.05; }  // WETH
        if asset_lower_hex.contains("7f39c5") { return 0.05; }  // wstETH
        if asset_lower_hex.contains("2f2a25") { return 0.05; }  // WBTC
        0.05
    }

    pub fn token_decimals(asset_lower_hex: &str) -> u32 {
        if asset_lower_hex.contains("2f2a25") { return 8; }    // WBTC
        if asset_lower_hex.contains("a0b869") { return 6; }    // USDC
        18
    }

    pub fn aave_ltv_liq_threshold(asset_lower_hex: &str) -> (f64, f64) {
        if asset_lower_hex.contains("c02aaa") { return (0.80, 0.825); } // WETH
        if asset_lower_hex.contains("2f2a25") { return (0.80, 0.825); } // WBTC
        (0.80, 0.825)
    }
}

/// Migration compatibility alias so shared modules referencing `base` resolve to Ethereum.
pub use ethereum as base;
