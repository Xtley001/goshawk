# Node Setup

*Production setup guide for self-hosted Base Mainnet IPC node infrastructure.*

This guide details building and provisioning a dedicated `op-geth` and `op-node` instance connected via local Unix Domain Socket (IPC) for sub-millisecond block latency.

## Prerequisites

- **Operating System:** Ubuntu 22.04 LTS
- **Hardware:** 8+ cores (3.5+ GHz), 32 GB RAM, 2 TB NVMe SSD, 1 Gbps unmetered network
- **Dependencies:** Go 1.21+, build-essential, git
- **L1 Endpoint:** Synced Ethereum Mainnet RPC (self-hosted Geth/Lighthouse or dedicated provider)

## Installation

### 1. Install Go

```bash
wget https://go.dev/dl/go1.21.6.linux-amd64.tar.gz
sudo tar -C /usr/local -xzf go1.21.6.linux-amd64.tar.gz
echo 'export PATH=$PATH:/usr/local/go/bin' >> ~/.bashrc && source ~/.bashrc
```

### 2. Build op-geth

```bash
git clone https://github.com/ethereum-optimism/op-geth.git && cd op-geth
git checkout $(git tag -l 'v*' | sort -V | tail -1)
make geth
sudo cp build/bin/geth /usr/local/bin/op-geth
cd ..
```

### 3. Build op-node

```bash
git clone https://github.com/ethereum-optimism/optimism.git && cd optimism
git checkout $(git tag -l 'op-node/v*' | sort -V | tail -1)
make op-node
sudo cp op-node/bin/op-node /usr/local/bin/op-node
cd ..
```

## Configuration

Create the shared JWT secret for engine API authentication:

```bash
sudo mkdir -p /var/run/base /data/base-geth
openssl rand -hex 32 | sudo tee /data/base-geth/jwt.txt > /dev/null
sudo chmod 600 /data/base-geth/jwt.txt
sudo chown -R ubuntu:ubuntu /data/base-geth /var/run/base
```

### Systemd Service: op-geth

Create `/etc/systemd/system/op-geth.service`:

```ini
[Unit]
Description=op-geth Base Mainnet Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
ExecStart=/usr/local/bin/op-geth \
  --datadir=/data/base-geth \
  --networkid=8453 \
  --http --http.api=eth,net,web3,debug,txpool \
  --http.addr=127.0.0.1 --http.port=8545 \
  --ws --ws.api=eth,net,web3,debug,txpool \
  --ws.addr=127.0.0.1 --ws.port=8546 \
  --ipcpath=/var/run/base/geth.ipc \
  --authrpc.addr=127.0.0.1 --authrpc.port=8551 \
  --authrpc.jwtsecret=/data/base-geth/jwt.txt \
  --syncmode=snap \
  --gcmode=archive \
  --cache=16384 \
  --metrics --metrics.addr=127.0.0.1 --metrics.port=6060
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

### Systemd Service: op-node

Create `/etc/systemd/system/op-node.service` (replace `L1_RPC_URL` with your Ethereum L1 RPC):

```ini
[Unit]
Description=op-node Base Mainnet Consensus Client
After=op-geth.service

[Service]
Type=simple
User=ubuntu
ExecStart=/usr/local/bin/op-node \
  --network=base-mainnet \
  --l1=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY \
  --l1.rpckind=basic \
  --l2=http://127.0.0.1:8551 \
  --l2.jwt-secret=/data/base-geth/jwt.txt \
  --rpc.addr=127.0.0.1 --rpc.port=9545 \
  --p2p.listen.tcp=9222 --p2p.listen.udp=9222
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Service Management

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now op-geth
sudo systemctl enable --now op-node
```

## Verification

Confirm the execution client is fully synced:

```bash
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
  http://127.0.0.1:8545
```

When sync is complete, the response will be `{"jsonrpc":"2.0","id":1,"result":false}`.
