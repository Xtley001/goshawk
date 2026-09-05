//! Corvus — Liquidation MEV System
//! Base Mainnet (Chain ID: 8453)
//!
//! Liquidation-only engine. All non-liquidation strategies (S1, S2, S4, S5, S6, S7) stripped.
#![allow(dead_code, non_upper_case_globals, unused_imports, unused_variables)]

mod config;
mod strategies;
mod shared;
mod monitoring;
mod chains;
mod flash;
mod swap;

use anyhow::Result;
use ethers::prelude::*;
use futures::StreamExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::RwLock;

use config::Config;
use shared::{
    addresses::ethereum,
    flash_loan::FlashLoanRouter,
    gas_oracle::GasOracle,
    mempool_monitor::MempoolMonitor,
    pool_discovery::PoolRegistry,
    position_indexer::PositionIndexer,
    price_feed::PriceFeed,
    price_oracle::PriceOracle,
    signing::Signer,
    simulation::SimulationEngine,
    submission::SubmissionPipeline,
};

async fn chain_breaker_tripped_alert(
    breaker:  &chains::ChainBreaker,
    chain_id: &str,
    telegram: &monitoring::TelegramAlerter,
) -> bool {
    if breaker.is_tripped() {
        let n = breaker.reverts.load(Ordering::SeqCst);
        let reason = format!("Chain '{}' breaker tripped after {} reverts (threshold {})", chain_id, n, breaker.threshold);
        tracing::error!("CIRCUIT BREAKER: {}", reason);
        telegram.alert_circuit_breaker(&reason).await;
        true
    } else {
        false
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,corvus=debug".into())
        )
        .init();

    tracing::info!("========================================");
    tracing::info!("Corvus Liquidation Engine v1.1 starting");
    tracing::info!("Chain: Ethereum Mainnet (1)");
    tracing::info!("Mode: Liquidation Only");
    tracing::info!("========================================");

    let cfg = Config::load()?;
    validate_config(&cfg)?;

    // Boot-time oracle gate enforcement (03_ADAPTER_ARCHITECTURE.md)
    let mut market_registry = chains::LendingMarketRegistry::new();
    let aave_addr: Address = cfg.get_chain("ethereum")
        .and_then(|c| c.get_address("aave_v3_pool_proxy"))
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    let morpho_addr: Address = cfg.get_chain("ethereum")
        .and_then(|c| c.get_address("morpho_blue"))
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    chains::register_lending_market(
        &mut market_registry,
        Box::new(chains::ethereum::aave_v3::EthereumAaveV3Adapter::new(aave_addr, Address::zero())),
    )?;
    chains::register_lending_market(
        &mut market_registry,
        Box::new(chains::ethereum::morpho_blue::MorphoBlueAdapter::new(morpho_addr)),
    )?;
    tracing::info!("Registered {} lending markets after SpotAmm validation", market_registry.len());

    monitoring::metrics::init();
    let metrics_port = cfg.metrics_port;
    tokio::spawn(async move {
        monitoring::metrics::serve(metrics_port).await;
    });

    let shutdown = Arc::new(tokio::sync::Notify::new());
    {
        let s = shutdown.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::warn!("SIGINT received — initiating graceful shutdown");
            s.notify_waiters();
        });
    }

    // Per-chain circuit breakers (03_ADAPTER_ARCHITECTURE.md §155)
    let mut breakers: std::collections::HashMap<String, Arc<chains::ChainBreaker>> = std::collections::HashMap::new();
    for ch in &cfg.chains {
        breakers.insert(ch.id.clone(), Arc::new(chains::ChainBreaker::new(ch.circuit_breaker_threshold)));
    }

    let eth_cfg = cfg.get_chain("ethereum").expect("ethereum chain config must exist in default.toml");
    let ipc_path = &eth_cfg.rpc_ipc_or_ws;
    let chain_id = eth_cfg.chain_id;
    let eth_price_fallback = eth_cfg.native_price_fallback_usd;
    let aave_provider_addr: Address = eth_cfg
        .get_address("aave_v3_pool_proxy")
        .unwrap_or_default()
        .parse()?;
    let executor_env = std::env::var("CORVUS_FLASH_EXECUTOR_ADDRESS").unwrap_or_default();
    let flash_executor_str = eth_cfg
        .get_address("flash_executor_address")
        .filter(|s| !s.is_empty())
        .unwrap_or(if executor_env.is_empty() {
            "0x0000000000000000000000000000000000000001"
        } else {
            &executor_env
        });
    let flash_executor_addr: Address = flash_executor_str.parse()?;
    let genesis = eth_cfg.bootstrap_from_block;

    tracing::info!("Connecting to IPC at {}...", ipc_path);
    let provider_blocks = Arc::new(
        Provider::<Ipc>::connect_ipc(ipc_path).await?
    );
    let provider_mempool = Arc::new(
        Provider::<Ipc>::connect_ipc(ipc_path).await?
    );
    let provider_archive = Arc::new(
        Provider::<Ipc>::connect_ipc(ipc_path).await?
    );

    let block_number = provider_blocks.get_block_number().await?;
    tracing::info!("Connected. Current block: {}", block_number);

    let aave_pool: Address = {
        let selector = &ethers::utils::keccak256(b"getPool()")[..4];
        let res = provider_blocks.call(
            &TransactionRequest {
                to:   Some(aave_provider_addr.into()),
                data: Some(selector.to_vec().into()),
                ..Default::default()
            }.into(),
            None,
        ).await?;
        Address::from_slice(&res[12..32])
    };
    tracing::info!("Aave V3 pool resolved: {:?}", aave_pool);

    let gas_oracle  = Arc::new(GasOracle::new(provider_blocks.clone(), &cfg));
    gas_oracle.refresh().await;

    let signer      = Arc::new(Signer::new(
        cfg.executor_private_key.expose(),
        chain_id,
        flash_executor_str,
        gas_oracle.clone(),
    )?);
    let sim_engine  = Arc::new(SimulationEngine::new(provider_archive.clone(), gas_oracle.clone(), cfg.clone()).await?);
    let flash_router= Arc::new(FlashLoanRouter::new(provider_blocks.clone(), &cfg)?);
    let submission  = Arc::new(SubmissionPipeline::new(&cfg, signer, provider_blocks.clone()).await?);
    let mempool_mon = Arc::new(MempoolMonitor::new(provider_mempool.clone(), eth_price_fallback)?);
    let pos_indexer = Arc::new(PositionIndexer::new(provider_archive.clone(), aave_pool));
    let price_oracle = Arc::new(PriceOracle::new(provider_blocks.clone(), eth_price_fallback)?);

    let telegram = Arc::new(monitoring::TelegramAlerter::new(
        cfg.telegram_bot_token.clone(),
        cfg.telegram_chat_id,
    ));

    tracing::info!("Building pool registry...");
    let registry = Arc::new(RwLock::new(PoolRegistry::build(provider_blocks.clone()).await?));
    tracing::info!("Pool registry: {} pools", registry.read().await.pool_count());

    let price_feed = PriceFeed::new(provider_blocks.clone(), registry.clone()).await?;

    tracing::info!("Bootstrapping positions from block {}...", genesis);
    pos_indexer.bootstrap(genesis, block_number.as_u64()).await?;
    tracing::info!("Bootstrap: {} positions", pos_indexer.position_count());

    let pi_live = pos_indexer.clone();
    tokio::spawn(async move {
        if let Err(e) = pi_live.watch_live().await {
            tracing::error!("Position indexer: {}", e);
        }
    });

    // ── Mempool fast-loop (Liquidation presign on pending oracle price updates) ──
    {
        let mm4      = mempool_mon.clone();
        let pi3      = pos_indexer.clone();
        let sim3     = sim_engine.clone();
        let fl3      = flash_router.clone();
        let sub3     = submission.clone();
        let cfg_mp   = cfg.clone();
        let prov_mp  = provider_mempool.clone();

        tokio::spawn(async move {
            let mut stream = match prov_mp.watch_pending_transactions().await {
                Ok(s)  => s,
                Err(e) => { tracing::error!("watch_pending_transactions: {}", e); return; }
            };
            while let Some(hash) = stream.next().await {
                let pending_prices = mm4.pending_oracle_prices();
                if !pending_prices.is_empty() {
                    let pi    = pi3.clone();
                    let si    = sim3.clone();
                    let fl    = fl3.clone();
                    let su    = sub3.clone();
                    let cfg_c = cfg_mp.clone();
                    let mm_c  = mm4.current_oracle_prices();
                    let eth   = mm4.eth_price();
                    let rvol  = mm4.realized_volatility();
                    tokio::spawn(async move {
                        if let Err(e) = strategies::liquidation::run_presign(pi, mm_c, pending_prices, si, fl, su, cfg_c, eth, rvol).await {
                            tracing::debug!("Liquidation presign: {}", e);
                        }
                    });
                }
                let _ = hash;
            }
        });
    }

    // ── Main confirmed-block loop ──────────────────────────────────────────
    tracing::info!("Subscribing to new blocks — liquidation engine active...");
    let mut block_stream = provider_blocks.subscribe_blocks().await?;

    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                tracing::warn!("Shutdown — exiting cleanly");
                break;
            }
            block_opt = block_stream.next() => {
                let block = match block_opt {
                    Some(b) => b,
                    None    => { tracing::warn!("Block stream ended"); break; }
                };
                let bn        = block.number.unwrap_or_default().as_u64();

                let block_recv_ts = std::time::Instant::now();

                if let Some(base_fee) = block.base_fee_per_gas {
                    gas_oracle.set_base_fee(base_fee.as_u64());
                    if bn % 5 == 0 {
                        gas_oracle.refresh().await;
                    }
                } else {
                    gas_oracle.refresh().await;
                }

                monitoring::metrics::update_ipc_latency(block_recv_ts.elapsed().as_secs_f64() * 1000.0);

                mempool_mon.flush_confirmed_pending(bn);

                const POOL_REFRESH_BLOCKS: u64 = 50;
                if bn % POOL_REFRESH_BLOCKS == 0 {
                    match PoolRegistry::build(provider_blocks.clone()).await {
                        Ok(fresh) => {
                            *registry.write().await = fresh;
                            tracing::debug!("Pool registry refreshed at block {}", bn);
                        }
                        Err(e) => tracing::warn!("Pool registry refresh failed: {}", e),
                    }
                }

                if let Err(e) = submission.resync_nonce().await {
                    tracing::warn!("Nonce resync: {}", e);
                }

                let mut pf = price_feed.clone();
                pf.update(bn, provider_blocks.clone()).await?;

                price_oracle.poll(&mempool_mon.current_oracle_prices()).await;

                if bn % cfg.monitoring_gas_check_interval_blocks == 0 {
                    if let Ok(balance) = provider_blocks.get_balance(
                        flash_executor_addr, None
                    ).await {
                        let balance_eth = balance.as_u128() as f64 / 1e18;
                        monitoring::metrics::update_gas_reserve(balance_eth);
                        if balance_eth < 1.0 {
                            let tg = telegram.clone();
                            let bal = balance_eth;
                            tokio::spawn(async move {
                                tg.alert_low_gas_reserve(bal).await;
                            });
                        }
                    }
                }

                let eth_oracle: Address = ethereum::CHAINLINK_ETH_USD.parse()
                    .expect("CHAINLINK_ETH_USD constant is malformed — fix addresses.rs");
                let eth_price = mempool_mon.current_oracle_prices()
                    .get(&eth_oracle).map(|p| *p)
                    .unwrap_or(eth_price_fallback);

                // ── Run Liquidation Strategy ──────────────────────────────
                let sim3  = sim_engine.clone();
                let fl3   = flash_router.clone();
                let sub3  = submission.clone();
                let cfg3  = cfg.clone();
                let pi3   = pos_indexer.clone();
                let mm3   = mempool_mon.clone();
                let ep3   = eth_price;
                let tg3   = telegram.clone();
                let eth_breaker = breakers.get("ethereum").cloned().unwrap_or_else(|| Arc::new(chains::ChainBreaker::new(10)));

                let s3 = tokio::spawn(async move {
                    if chain_breaker_tripped_alert(&eth_breaker, "ethereum", &tg3).await { return; }
                    let eff_gas = sim3.gas_oracle().effective_gas_price_wei();
                    if let Err(e) = strategies::liquidation::run(pi3, mm3, sim3, fl3, sub3, cfg3, ep3).await {
                        let revert_cost_usd = 250_000.0 * eff_gas / 1e18 * ep3;
                        eth_breaker.record_revert_with_cost(revert_cost_usd);
                        tracing::debug!("Liquidation error: {}", e);
                    } else {
                        eth_breaker.record_success();
                    }
                });

                let _ = tokio::time::timeout(std::time::Duration::from_millis(900), s3).await;
            }
        }
    }

    tracing::info!("Corvus exited cleanly.");
    Ok(())
}

fn validate_config(cfg: &Config) -> Result<()> {
    if cfg.chains.is_empty() {
        anyhow::bail!("Config must define at least one chain in [[chains]]");
    }
    if cfg.gas_estimate_safety_margin < 1.0 {
        anyhow::bail!("gas_estimate_safety_margin must be >= 1.0 (got {})", cfg.gas_estimate_safety_margin);
    }
    for ch in &cfg.chains {
        if ch.id.is_empty() {
            anyhow::bail!("Chain config entry has empty id");
        }
        for (name, val) in &ch.addresses.values {
            if !val.is_empty() {
                val.parse::<Address>().map_err(|e| {
                    anyhow::anyhow!("Chain '{}' invalid address for '{}': {}", ch.id, name, e)
                })?;
            }
        }
    }
    tracing::info!("Config validation passed — {} chains configured.", cfg.chains.len());
    Ok(())
}
