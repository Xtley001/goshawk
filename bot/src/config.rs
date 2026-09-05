//! SecretKey uses zeroize to wipe private key from heap on drop.
//! Goshawk configuration schema per 04_CONFIG_SCHEMA.md.

use serde::Deserialize;
use anyhow::Result;
use zeroize::Zeroize;
use std::collections::HashMap;

/// Wrapper that (a) prevents the key from appearing in logs/debug output,
/// (b) zeroes the backing String memory when dropped.
#[derive(Clone, Default)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn expose(&self) -> &str { &self.0 }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        // Zero the heap allocation before the allocator reclaims it.
        self.0.zeroize();
    }
}

impl<'de> Deserialize<'de> for SecretKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(SecretKey(String::deserialize(d)?))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub executor_private_key:  SecretKey,
    #[serde(default)]
    pub cold_wallet:            String,
    #[serde(default)]
    pub telegram_bot_token:     String,
    #[serde(default)]
    pub telegram_chat_id:       i64,
    pub metrics_port:           u16,
    pub rpc_timeout_secs:       u64,
    pub revm_sim_gas_limit:     u64,
    pub gas_estimate_safety_margin: f64,
    pub monitoring_gas_check_interval_blocks: u64,
    pub priority_fee_multiplier: f64,
    pub max_fee_base_multiplier: f64,
    pub min_profit_slippage_bps: u64,

    pub chains: Vec<ChainConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub id:                        String,   // "base", "hyperevm", "plasma", ...
    pub chain_id:                  u64,
    pub rpc_ipc_or_ws:              String,
    pub submission_mode:            SubmissionMode,
    pub lending_markets:            Vec<String>,   // adapter ids to register
    pub flash_priority:             Vec<String>,   // ordered fallback list
    pub swap_venues:                Vec<String>,
    pub hf_gate:                    HfGate,
    pub min_liquidation_profit_usd: f64,
    pub liquidation_swap_haircut:   f64,
    pub liquidation_safety_factor:  f64,
    pub bootstrap_from_block:       u64,
    pub native_price_fallback_usd:  f64,
    /// Protocol/router/oracle addresses this chain needs. Populated per-chain,
    /// never a shared global set — this is what let ADDR-CRIT-02 (mainnet
    /// Morpho market IDs mistakenly used on Base) happen in the first place.
    pub addresses:                  ChainAddresses,
    pub circuit_breaker_threshold:  u32,

    #[serde(default = "default_volatility_window")]
    pub volatility_window_blocks:   u64,
    #[serde(default = "default_volatility_scale")]
    pub volatility_scale:           f64,
    #[serde(default = "default_max_hf_widening")]
    pub max_hf_widening:           f64,
    #[serde(default = "default_opportunity_cost_usd")]
    pub opportunity_cost_usd:       f64,
}

fn default_volatility_window() -> u64 { 50 }
fn default_volatility_scale() -> f64 { 1.5 }
fn default_max_hf_widening() -> f64 { 0.05 }
fn default_opportunity_cost_usd() -> f64 { 15.0 }

impl ChainConfig {
    pub fn hf_threshold(&self) -> f64 {
        match self.hf_gate {
            HfGate::Flat { threshold } => threshold,
            HfGate::Staged { liquidatable, .. } => liquidatable,
        }
    }

    pub fn get_address(&self, key: &str) -> Option<&str> {
        self.addresses.values.get(key).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionMode {
    Shadow,
    DirectSequencer,
    /// Generalizes "private_builder" — carries a provider id so the same
    /// enum variant works across Flashbots (Ethereum), native private
    /// mempools (Polygon, Arbitrum Timeboost), and MEV-protected RPC
    /// endpoints (Base, BNB Chain via Alchemy/GetBlock/bloXroute), rather
    /// than assuming one relay shape fits every chain.
    Private { provider: PrivateSubmissionProvider },
}

impl SubmissionMode {
    pub fn is_shadow(&self) -> bool {
        matches!(self, SubmissionMode::Shadow)
    }

    pub fn is_private(&self) -> bool {
        matches!(self, SubmissionMode::Private { .. })
    }

    pub fn private_provider(&self) -> Option<&PrivateSubmissionProvider> {
        match self {
            SubmissionMode::Private { provider } => Some(provider),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateSubmissionProvider {
    FlashbotsProtect,        // Ethereum
    ArbitrumTimeboost,       // Arbitrum — native, effectively always-on
    PolygonPrivateMempool,   // Polygon — native, one RPC URL swap
    BloxrouteRelay,          // BNB Chain, and available on Polygon too
    MevProtectedRpc { endpoint: String }, // Base, BNB Chain via Alchemy/GetBlock
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum HfGate {
    Flat { threshold: f64 },
    Staged { presimulate: f64, submit: f64, liquidatable: f64 },
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChainAddresses {
    #[serde(flatten)]
    pub values: HashMap<String, String>,
}

impl Config {
    pub fn get_chain(&self, id: &str) -> Option<&ChainConfig> {
        self.chains.iter().find(|c| c.id == id)
    }

    pub fn ethereum_chain(&self) -> Option<&ChainConfig> {
        self.get_chain("ethereum")
    }

    pub fn base_chain(&self) -> Option<&ChainConfig> {
        self.get_chain("ethereum")
    }

    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();
        Ok(config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::Environment::with_prefix("GOSHAWK"))
            .build()?
            .try_deserialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_config() {
        let cfg = Config::load();
        assert!(cfg.is_ok(), "Config::load() must successfully parse default.toml: {:?}", cfg.err());
        let c = cfg.unwrap();
        assert_eq!(c.chains.len(), 1, "Default configuration must define exactly 1 chain (Ethereum)");
        
        let chain_ids: Vec<&str> = c.chains.iter().map(|ch| ch.id.as_str()).collect();
        assert_eq!(
            chain_ids,
            vec!["ethereum"]
        );
    }

    #[test]
    fn test_submission_mode_variants() {
        #[derive(Deserialize)]
        struct Wrapper {
            mode: SubmissionMode,
        }

        let s1: Wrapper = toml::from_str("mode = 'shadow'").unwrap();
        assert_eq!(s1.mode, SubmissionMode::Shadow);

        let s2: Wrapper = toml::from_str("mode = 'direct_sequencer'").unwrap();
        assert_eq!(s2.mode, SubmissionMode::DirectSequencer);

        let s3: Wrapper = toml::from_str("mode = { private = { provider = 'flashbots_protect' } }").unwrap();
        assert_eq!(s3.mode, SubmissionMode::Private { provider: PrivateSubmissionProvider::FlashbotsProtect });

        let s4: Wrapper = toml::from_str("mode = { private = { provider = 'arbitrum_timeboost' } }").unwrap();
        assert_eq!(s4.mode, SubmissionMode::Private { provider: PrivateSubmissionProvider::ArbitrumTimeboost });

        let s5: Wrapper = toml::from_str("mode = { private = { provider = 'polygon_private_mempool' } }").unwrap();
        assert_eq!(s5.mode, SubmissionMode::Private { provider: PrivateSubmissionProvider::PolygonPrivateMempool });

        let s6: Wrapper = toml::from_str("mode = { private = { provider = 'bloxroute_relay' } }").unwrap();
        assert_eq!(s6.mode, SubmissionMode::Private { provider: PrivateSubmissionProvider::BloxrouteRelay });

        let s7: Wrapper = toml::from_str("mode = { private = { provider = { mev_protected_rpc = { endpoint = 'https://example.com' } } } }").unwrap();
        assert_eq!(s7.mode, SubmissionMode::Private { provider: PrivateSubmissionProvider::MevProtectedRpc { endpoint: "https://example.com".into() } });
    }
}
