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
    #[serde(default)]
    expand: Option<bool>,
    #[serde(default)]
    article_ids: Option<Vec<String>>,
}

#[derive(JsonSchema, Deserialize)]
#[allow(dead_code)]
struct ExpandParams {
    /// Chunk IDs to expand (max 20)
    chunk_ids: Vec<u64>,
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
    pub async fn new(api_override: Option<String>) -> anyhow::Result<Self> {
        let cfg = config::load_config()?;
        let rpc_url = cfg.rpc_url;
        let api_base_url = api_override.unwrap_or(cfg.api_base_url);
        let sender = cfg.active_address;
        let platform_usdc_address = cfg.platform_usdc_address;

        let keypair = keystore::load_keypair(&cfg.keystore_path, &sender)?;

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
        description = "Search the Zing decentralized knowledge base. Provide short keyword queries (2-4 words preferred). Returns articles with relevance scores, excerpts, tags, and budget info. Default limit is 20."
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
        description = "Retrieve raw text segments from search results with per-chunk pricing. Provide short keyword queries. Returns chunks with text, scores, content_type, and truncation metadata. Set expand=true (no extra cost) to return full text instead of excerpts. Use article_ids to filter to specific articles. When truncation metadata is present, call zing_expand_chunks with those chunk_ids to retrieve full text. Default limit is 20."
    )]
    async fn zing_chunks(
        &self,
        Parameters(params): Parameters<ChunkParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tracing::info!("MCP zing_chunks q={} expand={:?}", params.q, params.expand);

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
            params.expand,
            params.article_ids.clone(),
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
                content_type: c.content_type.clone(),
                language: c.language.clone(),
                truncated: c.truncated.clone(),
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

    #[tool(
        description = "Expand truncated chunks to retrieve full untruncated text. Pass chunk_ids from chunks results that have non-null truncated fields. Max 20 chunk IDs per call."
    )]
    async fn zing_expand_chunks(
        &self,
        Parameters(params): Parameters<ExpandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tracing::info!("MCP zing_expand_chunks chunk_ids={:?}", params.chunk_ids);

        let chunk_ids_i64: Vec<i64> = params.chunk_ids.iter().map(|&id| id as i64).collect();

        let response = api::expand_chunks(
            &self.rpc_url,
            &self.api_base_url,
            &self.keypair,
            &self.sender,
            &self.platform_usdc_address,
            &chunk_ids_i64,
        )
        .await
        .map_err(|e| {
            tracing::error!("expand_chunks failed: {e}");
            ErrorData::internal_error(e.to_string(), None)
        })?;

        let agent_chunks: Vec<models::AgentExpandedChunk> = response
            .chunks
            .iter()
            .map(|c| models::AgentExpandedChunk {
                chunk_id: c.chunk_id,
                article_id: c.article_id.clone(),
                heading_path: c.heading_path.clone(),
                chunk_text: c.chunk_text.clone(),
                content_type: c.content_type.clone(),
                token_count: c.token_count,
                truncated: c.truncated.clone(),
            })
            .collect();

        let agent_response = models::AgentExpandResponse {
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
             Use zing_search to find articles, zing_chunks to retrieve semantic chunks, \
             and zing_expand_chunks to get full untruncated text for truncated chunks.",
        )
    }
}
