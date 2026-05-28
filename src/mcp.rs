use std::path::Path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData, ServerInfo};
use rmcp::transport::io::stdio;
use rmcp::{serve_server, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;

use crate::{api, config, keystore, models};

#[derive(JsonSchema, Deserialize)]
#[allow(dead_code)]
struct SearchParams {
    q: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(JsonSchema, Deserialize)]
#[allow(dead_code)]
struct ChunkParams {
    q: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[allow(dead_code)]
pub struct ZingMcpServer {
    rpc_url: String,
    api_base_url: String,
    keypair: Ed25519PrivateKey,
    sender: Address,
    platform_usdc_address: Address,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl ZingMcpServer {
    pub async fn new() -> anyhow::Result<Self> {
        let cfg = config::load_config()?;
        let rpc_url = cfg.rpc_url;
        let api_base_url = cfg.api_base_url;
        let sender = cfg.active_address;
        let platform_usdc_address = cfg.platform_usdc_address;

        let sui_config_dir = std::env::var("SUI_CONFIG_DIR")
            .unwrap_or_else(|_| format!("{}/.sui/sui_config", std::env::var("HOME").unwrap()));
        let keystore_path = Path::new(&sui_config_dir).join("sui.keystore");
        let keypair = keystore::load_keypair(&keystore_path, &sender)?;

        Ok(Self {
            rpc_url,
            api_base_url,
            keypair,
            sender,
            platform_usdc_address,
            tool_router: Self::tool_router(),
        })
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        tracing::info!("Starting Zing MCP server on stdio");
        let running = serve_server(self, stdio()).await?;
        running.waiting().await?;
        Ok(())
    }

    #[tool(
        description = "PRIMARY SEARCH GATEWAY: Use this tool first for any general inquiry, asset check, \
        or analytical question. \
        QUERY OPTIMIZATION RULE: Convert the user's natural language question into a short, high-density keyword string. \
        Strip out conversational filler words, dates, or vague outlook phrases (e.g., instead of searching \
        'what is the possible bitcoin market bottom prediction for late 2026', search 'Bitcoin market bottom analysis'). \
        This ensures maximum vector precision and clean lexical ranking scores."
    )]
    async fn zing_search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tracing::info!("MCP zing_search q={}", params.q);

        let wiki = "global".to_string();
        let owner_param = params.owner.as_deref();
        let limit = params.limit.unwrap_or(20).min(50);

        let response = api::search(
            &self.rpc_url,
            &self.api_base_url,
            &self.keypair,
            &self.sender,
            &self.platform_usdc_address,
            &params.q,
            &wiki,
            owner_param,
            limit,
        )
        .await
        .map_err(|e| {
            tracing::error!("search failed: {e}");
            ErrorData::internal_error(e.to_string(), None)
        })?;

        let agent_results: Vec<models::AgentSearchResult> = response
            .results
            .iter()
            .map(|r| {
                let excerpt = r.best_match.as_ref().map(|m| m.excerpt.clone());
                let heading_path = r
                    .best_match
                    .as_ref()
                    .map(|m| m.heading_path.clone())
                    .unwrap_or_default();
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
            })
            .collect();

        let agent_response = models::AgentSearchResponse {
            results: agent_results,
            budget: models::AgentBudget {
                paid_usdc: response.budget.paid_usdc,
                consumed_usdc: response.budget.consumed_usdc,
                remaining_usdc: response.budget.remaining_usdc,
            },
        };

        Ok(CallToolResult::structured(
            serde_json::to_value(&agent_response)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        ))
    }

    #[tool(
        description = "SURGICAL CHUNK RETRIEVAL: Use this tool to extract precise raw text segments and metadata \
        from specific articles or localized content spaces. \
        QUERY OPTIMIZATION RULE: Do not use natural language sentences, questions, or broad contextual phrases. \
        The query parameter must be a compact, 2-4 word keyword string targeting specific technical concepts, metrics, \
        or entities (e.g., instead of 'how does the 1-3 month realized price behave during a market bottom', \
        query '1-3m_RP market bottom'). Short, exact phrasing prevents vector dilution and guarantees \
        the highest similarity scores against raw document chunks."
    )]
    async fn zing_chunks(
        &self,
        Parameters(params): Parameters<ChunkParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tracing::info!("MCP zing_chunks q={}", params.q);

        let wiki = "global".to_string();
        let owner_param = params.owner.as_deref();
        let limit = params.limit.unwrap_or(20).min(50);

        let response = api::chunks(
            &self.rpc_url,
            &self.api_base_url,
            &self.keypair,
            &self.sender,
            &self.platform_usdc_address,
            &params.q,
            &wiki,
            owner_param,
            limit,
        )
        .await
        .map_err(|e| {
            tracing::error!("chunks failed: {e}");
            ErrorData::internal_error(e.to_string(), None)
        })?;

        let agent_chunks: Vec<models::AgentChunkResult> = response
            .chunks
            .iter()
            .map(|c| models::AgentChunkResult {
                chunk_id: c.chunk_id,
                article_id: c.article_id.clone(),
                title: c.title.clone(),
                text: c.text.clone(),
                score: c.scores.blended,
                chunk_token_count: c.chunk_token_count,
                heading_path: c.heading_path.clone(),
            })
            .collect();

        let agent_response = models::AgentChunksResponse {
            chunks: agent_chunks,
            budget: models::AgentBudget {
                paid_usdc: response.budget.paid_usdc,
                consumed_usdc: response.budget.consumed_usdc,
                remaining_usdc: response.budget.remaining_usdc,
            },
        };

        Ok(CallToolResult::structured(
            serde_json::to_value(&agent_response)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for ZingMcpServer {
    fn get_info(&self) -> ServerInfo {
        let capabilities = rmcp::model::ServerCapabilities::builder()
            .enable_tools()
            .build();
        rmcp::model::InitializeResult::new(capabilities).with_instructions(
            "Search the Zing decentralized knowledge base. \
             Use zing_search to find articles and zing_chunks to retrieve semantic chunks.",
        )
    }
}
