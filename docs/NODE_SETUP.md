# Node Setup

*Production setup guide for self-hosted Ethereum Mainnet (Chain ID 1) IPC node infrastructure.*

This guide details provisioning a dedicated high-performance Ethereum execution client (Geth) and consensus client (Lighthouse) connected via local Unix Domain Socket (IPC) for sub-millisecond block latency.

## Prerequisites

- **Operating System:** Ubuntu 22.04 / 24.04 LTS
- **Hardware:** 8+ cores (3.5+ GHz), 64 GB RAM, 2+ TB NVMe SSD (PCIe 4.0), 1 Gbps unmetered network
- **Dependencies:** Go 1.22+, build-essential, git, cmake, curl

## Installation

### 1. Build Geth (Execution Client)

```bash
git clone https://github.com/ethereum/go-ethereum.git && cd go-ethereum
git checkout $(git tag -l 'v*' | sort -V | tail -1)
make geth
sudo cp build/bin/geth /usr/local/bin/geth
cd ..
```

### 2. Install Lighthouse (Consensus Client)

```bash
curl -LO https://github.com/sigp/lighthouse/releases/latest/download/lighthouse-v5.3.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf lighthouse-v5.3.0-x86_64-unknown-linux-gnu.tar.gz
sudo cp lighthouse /usr/local/bin/lighthouse
```

## Configuration

Create dedicated directories and engine API JWT secret:

```bash
sudo mkdir -p /var/run/ethereum /data/ethereum-geth /data/ethereum-lighthouse
openssl rand -hex 32 | sudo tee /data/ethereum-geth/jwt.hex > /dev/null
sudo chmod 600 /data/ethereum-geth/jwt.hex
sudo chown -R ubuntu:ubuntu /data/ethereum-geth /data/ethereum-lighthouse /var/run/ethereum
```

### Systemd Service: Geth

Create `/etc/systemd/system/geth.service`:

```ini
[Unit]
Description=Geth Ethereum Mainnet Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
ExecStart=/usr/local/bin/geth \
  --datadir=/data/ethereum-geth \
  --mainnet \
  --http --http.api=eth,net,web3,debug,txpool \
  --http.addr=127.0.0.1 --http.port=8545 \
  --ws --ws.api=eth,net,web3,debug,txpool \
  --ws.addr=127.0.0.1 --ws.port=8546 \
  --ipcpath=/var/run/ethereum/geth.ipc \
  --authrpc.addr=127.0.0.1 --authrpc.port=8551 \
  --authrpc.jwtsecret=/data/ethereum-geth/jwt.hex \
  --syncmode=snap \
  --cache=16384 \
  --metrics --metrics.addr=127.0.0.1 --metrics.port=6060
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

### Systemd Service: Lighthouse

Create `/etc/systemd/system/lighthouse.service`:

```ini
[Unit]
Description=Lighthouse Ethereum Mainnet Consensus Client
After=geth.service
Wants=network-online.target

[Service]
Type=simple
User=ubuntu
ExecStart=/usr/local/bin/lighthouse bn \
  --network mainnet \
  --datadir /data/ethereum-lighthouse \
  --execution-endpoint http://127.0.0.1:8551 \
  --execution-jwt /data/ethereum-geth/jwt.hex \
  --checkpoint-sync-url https://mainnet.checkpoint.sigp.io \
  --http \
  --http-address 127.0.0.1 \
  --http-port 5052
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Service Management

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now geth
sudo systemctl enable --now lighthouse
```

## Verification

Confirm execution client is fully synced:

```bash
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
  http://127.0.0.1:8545
```

When sync is complete, the response will be `{"jsonrpc":"2.0","id":1,"result":false}`.

Validate block number and chain ID:

```bash
cast chain-id --rpc-url http://127.0.0.1:8545
cast block-number --rpc-url http://127.0.0.1:8545
```
