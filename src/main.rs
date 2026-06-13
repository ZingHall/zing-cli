use clap::{Parser, Subcommand};
use std::path::Path;

mod api;
mod config;
mod error;
mod keystore;
mod mcp;
mod models;
mod sui;

#[derive(Parser)]
#[command(
    name = "zing",
    about = "Zing platform CLI",
    version = env!("CARGO_PKG_VERSION"),
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the CLI version
    Version,

    /// Search across all indexed wikis
    Search {
        /// Search query
        q: String,
        /// Filter to a specific owner (omit for global search)
        #[arg(long)]
        owner: Option<String>,
        /// Max results (default: 20, max: 50)
        #[arg(long, default_value = "20")]
        limit: u32,
        /// Override API base URL
        #[arg(long)]
        api: Option<String>,
        /// Override Sui RPC URL
        #[arg(long)]
        rpc: Option<String>,
        /// Output as JSON for agent consumption
        #[arg(long)]
        json: bool,
    },
    /// Retrieve semantic chunks
    Chunks {
        /// Search query
        q: String,
        /// Filter to a specific owner (omit for global search)
        #[arg(long)]
        owner: Option<String>,
        /// Max results (default: 20, max: 50)
        #[arg(long, default_value = "20")]
        limit: u32,
        /// Return full untruncated text (no truncation)
        #[arg(long)]
        expand: bool,
        /// Override API base URL
        #[arg(long)]
        api: Option<String>,
        /// Override Sui RPC URL
        #[arg(long)]
        rpc: Option<String>,
        /// Output as JSON for agent consumption
        #[arg(long)]
        json: bool,
    },
    /// Expand truncated chunks — get full untruncated text
    Expand {
        /// Chunk IDs to expand (max 20)
        chunk_ids: Vec<u64>,
        /// Override API base URL
        #[arg(long)]
        api: Option<String>,
        /// Override Sui RPC URL
        #[arg(long)]
        rpc: Option<String>,
        /// Output as JSON for agent consumption
        #[arg(long)]
        json: bool,
    },
    /// Sui wallet queries
    Client {
        #[command(subcommand)]
        action: ClientAction,
    },
    /// MCP server commands
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
}

#[derive(Subcommand)]
enum ClientAction {
    /// Show the active Sui address
    ActiveAddress,
    /// Show SUI and USDC balances
    Balance,
}

#[derive(Subcommand)]
enum McpAction {
    /// Start the MCP server on stdio
    Serve {
        /// Override API base URL
        #[arg(long)]
        api: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Version => {
            println!("zing {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Search { q, owner, limit, api, rpc, json } => {
            run_search(q, owner, limit, api, rpc, json).await?;
        }
        Command::Chunks { q, owner, limit, expand, api, rpc, json } => {
            run_chunks(q, owner, limit, expand, api, rpc, json).await?;
        }
        Command::Expand { chunk_ids, api, rpc, json } => {
            run_expand(chunk_ids, api, rpc, json).await?;
        }
        Command::Client { action } => {
            run_client(action).await?;
        }
        Command::Mcp { action } => {
            match action {
                McpAction::Serve { api } => {
                    let server = mcp::ZingMcpServer::new(api).await?;
                    server.serve().await?;
                }
            }
        }
    }

    Ok(())
}

async fn run_expand(
    chunk_ids: Vec<u64>,
    api_override: Option<String>,
    rpc_override: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let cfg = config::load_config()?;
    let rpc_url = rpc_override.unwrap_or(cfg.rpc_url);
    let api_base_url = api_override.unwrap_or(cfg.api_base_url);

    // Convert Vec<u64> to Vec<i64> (BCS expects i64 in ExpandAccessMessage)
    let chunk_ids_i64: Vec<i64> = chunk_ids.iter().map(|&id| id as i64).collect();

    let sui_config_dir = std::env::var("SUI_CONFIG_DIR")
        .unwrap_or_else(|_| format!("{}/.sui/sui_config", std::env::var("HOME").unwrap()));
    let keystore_path = Path::new(&sui_config_dir).join("sui.keystore");
    let keypair = keystore::load_keypair(&keystore_path, &cfg.active_address)?;

    let response = api::expand_chunks(
        &rpc_url,
        &api_base_url,
        &keypair,
        &cfg.active_address,
        &cfg.platform_usdc_address,
        &chunk_ids_i64,
    )
    .await?;

    if json {
        let agent_chunks: Vec<models::AgentExpandedChunk> = response.chunks.iter().map(|c| {
            models::AgentExpandedChunk {
                chunk_id: c.chunk_id,
                article_id: c.article_id.clone(),
                heading_path: c.heading_path.clone(),
                chunk_text: c.chunk_text.clone(),
                content_type: c.content_type.clone(),
                token_count: c.token_count,
                truncated: c.truncated.clone(),
            }
        }).collect();

        let agent_response = models::AgentExpandResponse {
            chunks: agent_chunks,
            budget: models::AgentBudget {
                paid_usdc: response.budget.paid_usdc,
                consumed_usdc: response.budget.consumed_usdc,
                remaining_usdc: response.budget.remaining_usdc,
            },
        };

        println!("{}", serde_json::to_string_pretty(&agent_response)?);
        return Ok(());
    }

    println!("Expand: {} chunks returned", response.chunks.len());

    for (i, chunk) in response.chunks.iter().enumerate() {
        println!();
        println!("{}. Chunk {} — {} ({} tokens)", i + 1, chunk.chunk_id, chunk.content_type, chunk.token_count);
        if !chunk.heading_path.is_empty() {
            println!("   heading: {}", chunk.heading_path.join(" > "));
        }
        println!("   article: {}", chunk.article_id);
        println!("   text:");
        for line in chunk.chunk_text.lines() {
            println!("   {}", line);
        }
        if let Some(ref t) = chunk.truncated {
            match t.content_type.as_str() {
                "table" => {
                    if let (Some(total), Some(shown)) = (t.table_rows_total, t.table_rows_shown) {
                        println!("   (table: {} total rows, {} shown)", total, shown);
                    }
                }
                "code" => {
                    if let (Some(total), Some(shown)) = (t.code_lines_total, t.code_lines_shown) {
                        println!("   (code: {} total lines, {} shown)", total, shown);
                    }
                }
                "prose" => {
                    if let (Some(total), Some(shown)) = (t.prose_chars_total, t.prose_chars_shown) {
                        println!("   (prose: {} total chars, {} shown)", total, shown);
                    }
                }
                _ => {}
            }
        } else {
            println!("   (full text)");
        }
    }
    println!(
        "\nBudget: paid={}, consumed={}, remaining={}",
        response.budget.paid_usdc,
        response.budget.consumed_usdc,
        response.budget.remaining_usdc,
    );

    Ok(())
}

async fn run_client(action: ClientAction) -> anyhow::Result<()> {
    let cfg = config::load_config()?;
    let rpc_url = &cfg.rpc_url;

    match action {
        ClientAction::ActiveAddress => {
            println!("Active address: {}", cfg.active_address);
        }
        ClientAction::Balance => {
            let (addr_bal, coin_bal) =
                sui::get_usdc_balance(rpc_url, &cfg.active_address).await?;
            let total = addr_bal + coin_bal;

            println!("Active address: {}", cfg.active_address);
            println!(
                "USDC balance: {}.{:06} USDC",
                total / 1_000_000,
                total % 1_000_000
            );
        }
    }
    Ok(())
}

async fn run_search(
    q: String,
    owner: Option<String>,
    limit: u32,
    api_override: Option<String>,
    rpc_override: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let cfg = config::load_config()?;
    let rpc_url = rpc_override.unwrap_or(cfg.rpc_url);
    let api_base_url = api_override.unwrap_or(cfg.api_base_url);

    let sui_config_dir = std::env::var("SUI_CONFIG_DIR")
        .unwrap_or_else(|_| format!("{}/.sui/sui_config", std::env::var("HOME").unwrap()));
    let keystore_path = Path::new(&sui_config_dir).join("sui.keystore");
    let keypair = keystore::load_keypair(&keystore_path, &cfg.active_address)?;

    let wiki = owner.as_deref().unwrap_or("global").to_string();
    let owner_param = if owner.is_some() { owner.as_deref() } else { None };

    let response = api::search(
        &rpc_url,
        &api_base_url,
        &keypair,
        &cfg.active_address,
        &cfg.platform_usdc_address,
        &q,
        &wiki,
        owner_param,
        limit.min(50),
    )
    .await?;

    if json {
        let agent_results: Vec<models::AgentSearchResult> = response.results.iter().map(|r| {
            let excerpt = r.best_match.as_ref().map(|m| m.excerpt.clone());
            let heading_path = r.best_match.as_ref().map(|m| m.heading_path.clone()).unwrap_or_default();
            models::AgentSearchResult {
                article_id: r.article_id.clone(),
                title: r.title.clone().unwrap_or_else(|| "Untitled".into()),
                excerpt,
                heading_path,
                score: r.signals.relevance_score,
                article_token_count: r.signals.article_token_count,
                recency_days: r.signals.recency_days,
                tags: r.tags.clone(),
            }
        }).collect();

        let agent_response = models::AgentSearchResponse {
            results: agent_results,
            budget: models::AgentBudget {
                paid_usdc: response.budget.paid_usdc,
                consumed_usdc: response.budget.consumed_usdc,
                remaining_usdc: response.budget.remaining_usdc,
            },
        };

        println!("{}", serde_json::to_string_pretty(&agent_response)?);
        return Ok(());
    }

    println!("Search: \"{}\" — {} results", q, response.results.len());

    for (i, result) in response.results.iter().enumerate() {
        let title = result.title.as_deref().unwrap_or("Untitled");
        let score = result.signals.relevance_score;
        let recency = result.signals.recency_days;
        let tags = result.tags.join(" · ");
        let excerpt = result.best_match.as_ref().map(|m| m.excerpt.as_str()).unwrap_or("");
        let heading_path = result.best_match.as_ref().map(|m| m.heading_path.join(" > ")).unwrap_or_default();

        println!();
        println!("{}. {:<40} score: {:.2}  {}d ago", i + 1, title, score, recency);
        if !tags.is_empty() {
            println!("   tags: {}", tags);
        }
        if !excerpt.is_empty() {
            println!("   \"{}\"", excerpt);
        }
        if !heading_path.is_empty() {
            println!("   heading: {}", heading_path);
        }
        println!("   article: {}", result.article_id);
    }
    println!(
        "\nBudget: paid={}, consumed={}, remaining={}",
        response.budget.paid_usdc,
        response.budget.consumed_usdc,
        response.budget.remaining_usdc,
    );

    Ok(())
}

async fn run_chunks(
    q: String,
    owner: Option<String>,
    limit: u32,
    expand: bool,
    api_override: Option<String>,
    rpc_override: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let cfg = config::load_config()?;
    let rpc_url = rpc_override.unwrap_or(cfg.rpc_url);
    let api_base_url = api_override.unwrap_or(cfg.api_base_url);

    let sui_config_dir = std::env::var("SUI_CONFIG_DIR")
        .unwrap_or_else(|_| format!("{}/.sui/sui_config", std::env::var("HOME").unwrap()));
    let keystore_path = Path::new(&sui_config_dir).join("sui.keystore");
    let keypair = keystore::load_keypair(&keystore_path, &cfg.active_address)?;

    let wiki = owner.as_deref().unwrap_or("global").to_string();
    let owner_param = if owner.is_some() { owner.as_deref() } else { None };

    let response = api::chunks(
        &rpc_url,
        &api_base_url,
        &keypair,
        &cfg.active_address,
        &cfg.platform_usdc_address,
        &q,
        &wiki,
        owner_param,
        limit.min(50),
        if expand { Some(true) } else { None },
        None,
    )
    .await?;

    if json {
        let agent_chunks: Vec<models::AgentChunkResult> = response.chunks.iter().map(|c| {
            models::AgentChunkResult {
                chunk_id: c.chunk_id,
                article_id: c.article_id.clone(),
                title: c.title.clone(),
                text: c.text.clone(),
                score: c.scores.blended,
                chunk_token_count: c.chunk_token_count,
                heading_path: c.heading_path.clone(),
                content_type: c.content_type.clone(),
                language: c.language.clone(),
                truncated: c.truncated.clone(),
            }
        }).collect();

        let agent_response = models::AgentChunksResponse {
            chunks: agent_chunks,
            budget: models::AgentBudget {
                paid_usdc: response.budget.paid_usdc,
                consumed_usdc: response.budget.consumed_usdc,
                remaining_usdc: response.budget.remaining_usdc,
            },
        };

        println!("{}", serde_json::to_string_pretty(&agent_response)?);
        return Ok(());
    }

    println!("Chunks: \"{}\" — {} results", q, response.chunks.len());

    for (i, chunk) in response.chunks.iter().enumerate() {
        let text_preview = &chunk.text;

        println!();
        println!("{}. {:<40} score: {:.2}  {} tokens", i + 1, chunk.title, chunk.scores.blended, chunk.chunk_token_count);
        println!("   \"{}\"", text_preview);
        if !chunk.heading_path.is_empty() {
            println!("   heading: {}", chunk.heading_path.join(" > "));
        }
        println!("   chunk_id: {}  article: {}", chunk.chunk_id, chunk.article_id);
    }
    println!(
        "\nBudget: paid={}, consumed={}, remaining={}",
        response.budget.paid_usdc,
        response.budget.consumed_usdc,
        response.budget.remaining_usdc,
    );

    Ok(())
}
