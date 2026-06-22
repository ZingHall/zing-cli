# zing-cli

CLI tool + MCP server for paid semantic search and chunk retrieval on the [Zing](https://github.com/ZingHall/zing-aggregator) decentralized knowledge platform. Executes USDC micro-payments on Sui and queries the `zing-indexbind` API.

Two usage personas:

- **CLI user** — run `zing search`, `zing chunks`, `zing expand` directly in a terminal
- **MCP agent** — AI agents use `zing mcp serve` to get `zing_search`, `zing_chunks`, `zing_expand_chunks` tools over stdio

---

## Prerequisites

1. **Rust toolchain** — edition 2021
2. **Sui CLI** — [install](https://docs.sui.io/) and configure:
   ```bash
   sui client  # creates ~/.sui/sui_config/client.yaml
   ```
3. **Sui wallet** with USDC balance (≥0.01 USDC)

---

## Installation

### Option 1: Install from GitHub

```bash
cargo install --git https://github.com/ZingHall/zing-cli.git zing-cli
```

### Option 2: Clone and build

```bash
git clone https://github.com/ZingHall/zing-cli.git
cd zing-cli
cargo install --path . --bin zing
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
# → zing 0.1.0
```

---

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

---

## CLI Commands

### `zing version`

Show the CLI version.

```bash
zing version
```

---

### `zing search` — Paid semantic search

Search across all indexed wikis.

```bash
# Basic search
zing search "what is blockchain"

# Scoped to a specific creator's wiki
zing search "Move language" --owner 0x1aa2c40369fa0fffb12fe6e1415b8aba52d15cc3cf59e001adc5d2687920fbd6

# Custom result count (default: 20, max: 50)
zing search "DeFi" --limit 30

# Override endpoints
zing search "zk proofs" --api https://staging.api.com --rpc https://fullnode.mainnet.sui.io:443

# JSON output
zing search "Sui gas" --json
```

| Flag | Type | Description |
|------|------|-------------|
| `--owner` | string | Filter to a specific creator's wiki |
| `--limit` | int | Max results (default: 20, max: 50) |
| `--api` | string | Override API base URL |
| `--rpc` | string | Override Sui fullnode URL |
| `--json` | bool | Output JSON for agent consumption |

---

### `zing chunks` — Paid chunk retrieval

Retrieve semantic chunks with per-chunk pricing. Returns precise raw text segments with metadata.

```bash
# Basic chunk retrieval
zing chunks "zero-knowledge proofs"

# Return full untruncated text (no excerpts)
zing chunks "consensus" --expand

# Scoped with custom limit
zing chunks "Move language" --owner 0xabc... --limit 30

# JSON output
zing chunks "smart contracts" --json
```

| Flag | Type | Description |
|------|------|-------------|
| `--owner` | string | Filter to a specific creator's wiki |
| `--limit` | int | Max results (default: 20, max: 50) |
| `--expand` | bool | Return full untruncated chunk text (no excerpts) |
| `--api` | string | Override API base URL |
| `--rpc` | string | Override Sui fullnode URL |
| `--json` | bool | Output JSON for agent consumption |

---

### `zing expand` — Expand truncated chunks

Retrieve the full untruncated text for up to 20 chunks by their IDs. Use this when a chunk result shows truncation metadata.

```bash
# Expand specific chunk IDs
zing expand 94 1603 1802

# JSON output
zing expand 94 1603 --json
```

| Flag | Type | Description |
|------|------|-------------|
| `--api` | string | Override API base URL |
| `--rpc` | string | Override Sui fullnode URL |
| `--json` | bool | Output JSON for agent consumption |

---

### `zing client` — Wallet utilities

```bash
# Show the active Sui address
zing client active-address

# Show SUI and USDC balances
zing client balance
```

---

### `zing mcp serve` — Start MCP server

Start an MCP server on stdio for AI agent integration. Exposes `zing_search`, `zing_chunks`, and `zing_expand_chunks` as MCP tools. See [MCP Server](#mcp-server) for full documentation.

```bash
zing mcp serve

# With API override
zing mcp serve --api https://staging.api.com
```

| Flag | Type | Description |
|------|------|-------------|
| `--api` | string | Override API base URL |

---

## JSON Output Schemas

The `--json` flag on `search`, `chunks`, and `expand` commands returns structured JSON instead of human-readable output. All monetary values are in micro-USDC (1 USDC = 1,000,000 micro-USDC).

### Search (`--json`)

```json
{
  "results": [
    {
      "article_id": "0xf6bd...",
      "title": "Gas Fees",
      "excerpt": "When you submit transactions...",
      "heading_path": ["Introduction", "Gas Fees"],
      "score": 0.81,
      "article_token_count": 4200,
      "recency_days": 0,
      "tags": ["cryptocurrency", "defi"]
    }
  ],
  "budget": {
    "paid_usdc": "10000",
    "consumed_usdc": "1250",
    "remaining_usdc": "8750"
  }
}
```

### Chunks (`--json`)

```json
{
  "chunks": [
    {
      "chunk_id": 94,
      "article_id": "0xf6bd...",
      "title": "What is Web3",
      "text": "Web3 is the next generation of the internet...",
      "score": 2.14,
      "chunk_token_count": 116,
      "heading_path": ["Introduction"],
      "content_type": "prose",
      "language": null,
      "truncated": {
        "content_type": "prose",
        "prose_chars_total": 850,
        "prose_chars_shown": 280
      }
    }
  ],
  "budget": {
    "paid_usdc": "10000",
    "consumed_usdc": "797",
    "remaining_usdc": "9203"
  }
}
```

**Truncation:** When `truncated` is non-null, the `text` field is an excerpt. The `truncated` object tells you what was omitted:

| Field | When | Meaning |
|-------|------|---------|
| `content_type` | always | `"prose"`, `"code"`, or `"table"` |
| `table_rows_total / table_rows_shown` | table chunks | Hidden data rows |
| `code_lines_total / code_lines_shown` | code chunks | Truncated lines |
| `prose_chars_total / prose_chars_shown` | prose chunks | Truncated characters |

Use `zing expand <chunk_id>` or the `zing_expand_chunks` MCP tool to retrieve the full text.

### Expand (`--json`)

```json
{
  "chunks": [
    {
      "chunk_id": 94,
      "article_id": "0xf6bd...",
      "heading_path": ["Introduction", "What is Web3"],
      "chunk_text": "Web3 is the next generation of the internet...\n\n(full untruncated text)",
      "content_type": "prose",
      "token_count": 116,
      "truncated": {
        "content_type": "table",
        "table_rows_total": 15,
        "table_rows_shown": 3
      }
    }
  ],
  "budget": {
    "paid_usdc": "10000",
    "consumed_usdc": "2320",
    "remaining_usdc": "7680"
  }
}
```

---

## MCP Server

`zing mcp serve` starts an MCP server over stdio that AI agents (Claude Desktop, Cursor, Codex, etc.) can connect to as a subprocess. The server provides three tools.

### Client Setup

**Claude Desktop** — add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "zing": {
      "command": "zing",
      "args": ["mcp", "serve"]
    }
  }
}
```

**Cursor** — add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "zing": {
      "command": "zing",
      "args": ["mcp", "serve"]
    }
  }
}
```

**With API override:**

```json
{
  "command": "zing",
  "args": ["mcp", "serve", "--api", "https://staging.api.com"]
}
```

---

### Tool: `zing_search`

**Description:**

> Search the Zing decentralized knowledge base. Provide short keyword queries (2-4 words preferred). Returns articles with relevance scores, excerpts, tags, and budget info. Default limit is 20.

**Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `q` | string | yes | — | Search query (compact keywords preferred) |
| `owner` | string | no | `null` | Filter to specific creator's wiki address |
| `limit` | int | no | `20` | Max results (capped at 50) |

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `results[].article_id` | string | On-chain article address |
| `results[].title` | string | Article title |
| `results[].excerpt` | string or null | Best-matching text snippet |
| `results[].heading_path` | string[] | Heading hierarchy for the match |
| `results[].score` | float | Relevance score (cross-encoder reranked) |
| `results[].article_token_count` | int | Total tokens in the article |
| `results[].recency_days` | int | Days since last index |
| `results[].tags` | string[] | Extracted topic tags |
| `budget.paid_usdc` | string | Total USDC sent (in micro-USDC) |
| `budget.consumed_usdc` | string | USDC consumed by this request |
| `budget.remaining_usdc` | string | USDC remaining after this request |

---

### Tool: `zing_chunks`

**Description:**

> Retrieve raw text segments from search results with per-chunk pricing. Provide short keyword queries. Returns chunks with text, scores, content_type, and truncation metadata. Set expand=true (no extra cost) to return full text instead of excerpts. Use article_ids to filter to specific articles. When truncation metadata is present, call zing_expand_chunks with those chunk_ids to retrieve full text. Default limit is 20.

**Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `q` | string | yes | — | Search query (compact 2-4 word keywords) |
| `owner` | string | no | `null` | Filter to specific creator's wiki address |
| `limit` | int | no | `20` | Max results (capped at 50) |
| `expand` | bool | no | `false` | Return full untruncated text instead of excerpts (no extra cost) |
| `article_ids` | string[] | no | `null` | Filter to specific article IDs |

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `chunks[].chunk_id` | int | Unique chunk identifier |
| `chunks[].article_id` | string | On-chain article address |
| `chunks[].title` | string | Article title |
| `chunks[].text` | string | Chunk text (excerpt or full if `expand=true`) |
| `chunks[].score` | float | Blended relevance score |
| `chunks[].chunk_token_count` | int | Estimated tokens in this chunk |
| `chunks[].heading_path` | string[] | Heading hierarchy |
| `chunks[].content_type` | string | `"prose"`, `"code"`, or `"table"` |
| `chunks[].language` | string or null | Programming language for code chunks |
| `chunks[].truncated` | object or null | Truncation metadata (see below) |
| `budget.*` | string | Same structure as `zing_search` |

**Truncation workflow:**

```
zing_chunks → chunk.truncated is non-null → zing_expand_chunks(chunk_ids) → full text
```

---

### Tool: `zing_expand_chunks`

**Description:**

> Expand truncated chunks to retrieve full untruncated text. Pass chunk_ids from chunks results that have non-null truncated fields. Max 20 chunk IDs per call.

**Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `chunk_ids` | int[] | yes | — | Chunk IDs to expand (max 20) |

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `chunks[].chunk_id` | int | Chunk identifier |
| `chunks[].article_id` | string | On-chain article address |
| `chunks[].heading_path` | string[] | Heading hierarchy |
| `chunks[].chunk_text` | string | Full untruncated chunk content |
| `chunks[].content_type` | string | `"prose"`, `"code"`, or `"table"` |
| `chunks[].token_count` | int | Estimated tokens |
| `chunks[].truncated` | object or null | Truncation metadata (always non-null for expand) |
| `budget.*` | string | Same structure as `zing_search` |

---

## Payment Flow

Each request costs a flat search/infrastructure fee (default 0.0005 USDC for ≤20 results, scales linearly for larger limits) plus per-result/per-chunk token fees:

1. **Send USDC** — the CLI sends USDC via `0x2::balance::send_funds<USDC>` to the platform address. If balance is insufficient, it auto-consolidates USDC coins first.
2. **Sign message** — BCS-encodes `ApiAccessMessage {q, wiki, transaction_digest, timestamp, expand?, article_ids?}` and signs as a `PersonalMessage` with the Ed25519 keypair from `sui.keystore`.
3. **Submit** — sends the signed request to the indexbind API. The server verifies the on-chain payment, runs the search/chunk pipeline, and returns results up to the paid budget.

The budget breakdown is shown in all output:

```
Budget: paid=10000, consumed=1250, remaining=8750
```

---

## Project Structure

```
zing-cli/
  Cargo.toml        — workspace root (zing-cli + zing-eval)
  README.md
  src/
    main.rs         — CLI entry: clap subcommands, output formatting
    lib.rs          — module declarations
    config.rs       — reads Sui client.yaml + env vars
    error.rs        — typed error codes
    keystore.rs     — loads Ed25519 keypair from sui.keystore
    sui.rs          — USDC balance check + payment PTB builder
    api.rs          — ApiAccessMessage signing + HTTP calls
    models.rs       — request/response types (serde)
    mcp.rs          — MCP server (zing_search, zing_chunks, zing_expand_chunks tools)
  eval/             — zing-eval RAG evaluation framework
    src/
      main.rs       — eval CLI (run, list, formula)
      checks.rs     — L1 retrieval / L2 score sanity
      golden.rs     — YAML query loader
      runner.rs     — API client for chunk/search estimate
      l3_eval.rs    — LLM judge (opt-in)
      report.rs     — JSON report writer
      types.rs      — shared types
    queries/        — golden query YAML definitions
```
