# zing-cli

CLI tool for paid semantic search and chunk retrieval on the Zing platform.
Executes USDC micro-payments on Sui and queries the [zing-indexbind](https://github.com/ZingHall/zing-aggregator) API.

## Prerequisites

1. **Rust toolchain** — edition 2021
2. **Sui CLI** — [install](https://docs.sui.io/) and configure:
   ```bash
   sui client  # creates ~/.sui/sui_config/client.yaml
   ```
3. **Sui wallet** with USDC balance (≥0.01 USDC) on mainnet

## Installation

### Option 1: Install from GitHub

```bash
cargo install --git https://github.com/ZingHall/zing-cli.git
```

### Option 2: Clone and build

```bash
git clone https://github.com/ZingHall/zing-cli.git
cd zing-cli
cargo install --path .
```

### Option 3: Build in place

```bash
cargo build --release -p zing-cli
./target/release/zing version
```

The binary `zing` is placed in `~/.cargo/bin/` — ensure it's on your `PATH`.

### Verify

```bash
zing version
```

If you see a version string like `0.1.0`, installation succeeded.

## Configuration

### How the CLI finds your Sui wallet

The CLI reads from your Sui configuration directory:

```
~/.sui/sui_config/
  client.yaml     — active address (created by `sui client`)
  sui.keystore    — Ed25519 keypair (created by `sui client`)
```

Override the directory with the `SUI_CONFIG_DIR` environment variable.

### Defaults and overrides

| Variable | Overrides | Default |
|----------|-----------|---------|
| `ZING_API_URL` | API base URL | `https://search.zing.services` |
| `ZING_PLATFORM_USDC_ADDRESS` | Payment recipient | hardcoded platform address |
| `SUI_CONFIG_DIR` | Sui config path | `~/.sui/sui_config` |
| `--api` flag | API base URL | overrides env/fallback |
| `--rpc` flag | Sui fullnode URL | `https://fullnode.mainnet.sui.io:443` |

The RPC URL defaults to **mainnet**. For testnet, pass `--rpc https://fullnode.testnet.sui.io:443`.

## Usage

### `zing version`

Show the CLI version.

```bash
zing version
# → zing 0.1.0
```

### `zing search <query>`

Paid semantic search across all indexed wikis.

```bash
# Basic search
zing search "what is blockchain"

# Scoped to a specific creator's wiki
zing search "Move language" --owner 0x1aa2c40369fa0fffb12fe6e1415b8aba52d15cc3cf59e001adc5d2687920fbd6

# Custom result count (default: 20, max: 50)
zing search "DeFi" --limit 30

# Override API and RPC endpoints
zing search "zk proofs" --api https://staging.api.com --rpc https://fullnode.mainnet.sui.io:443

# JSON output for agent consumption
zing search "Sui gas" --json
```

### `zing chunks <query>`

Paid chunk retrieval with full content text.

```bash
# Basic chunk retrieval
zing chunks "zero-knowledge proofs"

# Scoped with custom limit
zing chunks "consensus" --owner 0xabc... --limit 20

# JSON output
zing chunks "smart contracts" --json
```

### `zing client`

Sui wallet utilities.

```bash
# Show the active Sui address
zing client active-address

# Show SUI and USDC balances
zing client balance
```

### `zing mcp serve`

Start an MCP server on stdio for AI agent integration.
Provides `zing_search` (triage/search) and `zing_chunks` (deep content retrieval) as MCP tools.

```bash
zing mcp serve
```

Connect an MCP-compatible client (e.g., Claude Desktop, Cursor) by running this command as a subprocess.

### Flags

All search and chunk commands support:

| Flag | Type | Description |
|------|------|-------------|
| `--owner` | string | Filter to a specific wiki (omit for global search) |
| `--limit` | int | Max results (default: 20, max: 50) |
| `--api` | string | Override API base URL |
| `--rpc` | string | Override Sui fullnode URL |
| `--json` | bool | Output JSON for agent consumption |

## JSON Output (Agent Consumption)

The `--json` flag returns structured JSON instead of human-readable output:

```bash
zing search "Sui gas mechanism" --json | jq '.results[:1] | {title, score, excerpt}'
```

Search response:
```json
{
  "results": [
    {
      "article_id": "0xabc...",
      "title": "Gas Fees",
      "excerpt": "When you submit transactions...",
      "score": 0.81,
      "token_count": 4200,
      "recency_days": 0,
      "tags": ["finance", "cryptocurrency"]
    }
  ],
  "budget": {
    "paid_usdc": 10000,
    "consumed_usdc": 1250,
    "remaining_usdc": 8750
  }
}
```

## Payment Flow

Each request costs the flat search fee (default 0.0005 USDC for ≤20 results, scales linearly for larger limits) plus per-result token fees:

1. **Send USDC** — the CLI sends USDC via `0x2::balance::send_funds<USDC>` to the platform address. If balance is insufficient, it auto-consolidates USDC coins first.
2. **Sign message** — BCS-encodes `ApiAccessMessage {q, wiki, transaction_digest, timestamp}` and signs as a `PersonalMessage` with the Ed25519 keypair from `sui.keystore`.
3. **Submit** — sends the signed request to the indexbind API. The server verifies the on-chain payment, runs the search pipeline, and returns results up to the paid budget.

The budget breakdown is shown in the output:

```
Budget: paid=10000, consumed=1250, remaining=8750
```

## Project Structure

```
zing-cli/
  Cargo.toml
  README.md
  src/
    main.rs        — CLI entry: clap subcommands, output formatting
    lib.rs         — module declarations
    config.rs      — reads Sui client.yaml + env vars
    error.rs       — typed error codes
    keystore.rs    — loads Ed25519 keypair from sui.keystore
    sui.rs         — USDC balance check + payment PTB builder
    api.rs         — ApiAccessMessage signing + HTTP calls
    models.rs      — request/response types (serde)
    mcp.rs         — MCP server (zing_search, zing_chunks tools)
```
