//! Private submission pipeline for Ethereum Mainnet builder auction.
//! 09_SUBMISSION_AND_SIMULATION.md §9.3–9.5.

use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use crate::{
    config::{Config, SubmissionMode},
    shared::signing::Signer,
};

/// Canonical Ethereum builder relays (09_SUBMISSION_AND_SIMULATION.md §9.3)
pub const FLASHBOTS_RELAY_ENDPOINT: &str = "https://relay.flashbots.net";
pub const TITAN_BUILDER_ENDPOINT:   &str = "https://rpc.titanbuilder.xyz";
pub const BEAVER_BUILD_ENDPOINT:    &str = "https://rpc.beaverbuild.com";

pub struct SubmissionPipeline {
    signer:               Arc<Signer>,
    provider:             Arc<Provider<Ipc>>,
    builder_endpoints:    Vec<String>,
    active_builder_idx:   Arc<AtomicUsize>,
    consecutive_failures: Arc<AtomicUsize>,
    submission_mode:      SubmissionMode,
    client:               reqwest::Client,
    nonce:                Arc<AtomicU64>,
}

impl SubmissionPipeline {
    pub async fn new(cfg: &Config, signer: Arc<Signer>, provider: Arc<Provider<Ipc>>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .timeout(std::time::Duration::from_secs(cfg.rpc_timeout_secs))
            .build()?;

        let wallet_addr = signer.wallet_address();
        let nonce = provider
            .get_transaction_count(wallet_addr, Some(BlockId::Number(BlockNumber::Pending)))
            .await?.as_u64();
        tracing::info!("Nonce bootstrap: wallet={:?} nonce={}", wallet_addr, nonce);

        let eth_chain = cfg.get_chain("ethereum");
        let submission_mode = eth_chain
            .map(|c| c.submission_mode.clone())
            .unwrap_or(SubmissionMode::Shadow);

        // Relay list reduced strictly to the three Ethereum builders (09 §9.3)
        let builder_endpoints = if let Ok(ep) = std::env::var("GOSHAWK_BUILDER_ENDPOINTS")
            .or_else(|_| std::env::var("CORVUS_BUILDER_ENDPOINTS"))
        {
            ep.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        } else {
            vec![
                FLASHBOTS_RELAY_ENDPOINT.to_string(),
                TITAN_BUILDER_ENDPOINT.to_string(),
                BEAVER_BUILD_ENDPOINT.to_string(),
            ]
        };

        Ok(Self {
            signer,
            provider,
            builder_endpoints,
            active_builder_idx: Arc::new(AtomicUsize::new(0)),
            consecutive_failures: Arc::new(AtomicUsize::new(0)),
            submission_mode,
            client,
            nonce: Arc::new(AtomicU64::new(nonce)),
        })
    }

    pub fn builder_endpoints(&self) -> &[String] {
        &self.builder_endpoints
    }

    pub fn active_builder_index(&self) -> usize {
        self.active_builder_idx.load(Ordering::SeqCst)
    }

    pub async fn submit(&self, calldata: Bytes, gas_limit: u64) -> Result<H256> {
        self.submit_with_tip(calldata, gas_limit, 1.0).await
    }

    pub async fn submit_priority(&self, calldata: Bytes, gas_limit: u64) -> Result<H256> {
        self.submit_with_tip(calldata, gas_limit, 2.5).await
    }

    pub async fn submit_with_escalation(
        &self, calldata: Bytes, gas_limit: u64, max_blocks: u8,
    ) -> Result<H256> {
        let nonce = self.nonce.fetch_add(1, Ordering::SeqCst);
        let mut last = H256::zero();
        for attempt in 0..max_blocks {
            let (raw, hash) = self.signer.sign_tx_escalated(calldata.clone(), gas_limit, attempt, nonce).await?;
            self.broadcast_raw(&raw).await;
            last = hash;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if let Ok(Some(_)) = self.provider.get_transaction_receipt(hash).await {
                tracing::info!("Tx included after {} attempt(s): {:?}", attempt + 1, hash);
                crate::monitoring::metrics::record_inclusion();
                self.consecutive_failures.store(0, Ordering::SeqCst);
                return Ok(hash);
            }
        }
        Ok(last)
    }

    pub async fn resync_nonce(&self) -> Result<()> {
        let wallet_addr = self.signer.wallet_address();
        let chain_nonce = self.provider
            .get_transaction_count(wallet_addr, Some(BlockId::Number(BlockNumber::Pending)))
            .await?.as_u64();
        let local = self.nonce.load(Ordering::SeqCst);
        if chain_nonce > local {
            self.nonce.store(chain_nonce, Ordering::SeqCst);
            tracing::info!("Nonce resynced: {} → {}", local, chain_nonce);
        }
        Ok(())
    }

    async fn submit_with_tip(&self, calldata: Bytes, gas_limit: u64, tip_mult: f64) -> Result<H256> {
        let nonce = self.nonce.fetch_add(1, Ordering::SeqCst);
        let (raw, hash) = self.signer.sign_tx_with_nonce(calldata, gas_limit, tip_mult, nonce).await?;
        self.broadcast_raw(&raw).await;
        crate::monitoring::metrics::record_submission();
        Ok(hash)
    }

    async fn broadcast_raw(&self, raw: &Bytes) {
        if self.submission_mode == SubmissionMode::Shadow {
            tracing::debug!("Shadow mode active: raw tx broadcast skipped");
            return;
        }

        if self.builder_endpoints.is_empty() {
            tracing::warn!("No builder endpoints configured for private submission");
            return;
        }

        let raw_hex = format!("0x{}", hex::encode(raw.as_ref()));
        let payload = serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "eth_sendRawTransaction",
            "params": [&raw_hex]
        });

        // 09 §9.3 & §9.5: Broadcast to all 3 builder endpoints in parallel, with failover tracking.
        let futs: Vec<_> = self.builder_endpoints.iter().map(|ep| {
            let c = self.client.clone();
            let p = payload.clone();
            let ep = ep.clone();
            async move {
                match c.post(&ep).json(&p).send().await {
                    Ok(resp) => {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if let Some(err) = body.get("error") {
                                tracing::warn!("Builder relay error from {}: {}", ep, err);
                                false
                            } else {
                                true
                            }
                        } else {
                            false
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Broadcast failed to {}: {}", ep, e);
                        false
                    }
                }
            }
        }).collect();

        let results = futures::future::join_all(futs).await;
        let any_success = results.iter().any(|&ok| ok);

        if !any_success {
            let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
            tracing::warn!("All builder relays failed to accept bundle (consecutive: {})", failures);
            // 09 §9.5: Bundle relay fails to land 3 consecutive bundles -> Failover to next relay
            if failures >= 3 {
                let next_idx = (self.active_builder_idx.load(Ordering::SeqCst) + 1) % self.builder_endpoints.len();
                self.active_builder_idx.store(next_idx, Ordering::SeqCst);
                self.consecutive_failures.store(0, Ordering::SeqCst);
                tracing::warn!(
                    "Triggered builder failover: rotated active primary relay to index {} ({})",
                    next_idx,
                    self.builder_endpoints[next_idx]
                );
            }
        } else {
            self.consecutive_failures.store(0, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethereum_builder_relay_constants() {
        assert_eq!(FLASHBOTS_RELAY_ENDPOINT, "https://relay.flashbots.net");
        assert_eq!(TITAN_BUILDER_ENDPOINT, "https://rpc.titanbuilder.xyz");
        assert_eq!(BEAVER_BUILD_ENDPOINT, "https://rpc.beaverbuild.com");

        let default_relays = [
            FLASHBOTS_RELAY_ENDPOINT,
            TITAN_BUILDER_ENDPOINT,
            BEAVER_BUILD_ENDPOINT,
        ];
        assert_eq!(default_relays.len(), 3);
    }
}
