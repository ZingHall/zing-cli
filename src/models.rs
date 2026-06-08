#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── Request ──

#[derive(Serialize)]
pub struct PaidRequest {
    pub q: String,
    pub wiki: String,
    pub owner: Option<String>,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<bool>,
    pub transaction_digest: String,
    pub signature: String,
    pub bytes: String,
}

// ── Search Response ──

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub query_text: String,
    pub wiki_scope: String,
    pub results: Vec<SearchResult>,
    pub budget: BudgetBreakdown,
    pub payments: Vec<PaymentLine>,
}

#[derive(Deserialize, Debug)]
pub struct SearchResult {
    pub article_id: String,
    pub relative_path: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub best_match: Option<BestMatch>,
    pub chunk_token_count: u32,
    pub raw_vector_score: Option<f64>,
    pub raw_lexical_score: Option<f64>,
    pub tags: Vec<String>,
    pub signals: Signals,
}

#[derive(Deserialize, Debug)]
pub struct BestMatch {
    pub excerpt: String,
    pub heading_path: Vec<String>,
    pub char_start: u32,
    pub char_end: u32,
}

#[derive(Deserialize, Debug)]
pub struct Signals {
    pub relevance_score: f64,
    pub article_token_count: u32,
    pub recency_days: u32,
    pub tag_confidence: f64,
    pub wiki_file_count: u32,
    pub primary_tag: String,
}

// ── Chunks Response ──

#[derive(Deserialize, Debug)]
pub struct ChunksResponse {
    pub query_text: String,
    pub wiki_scope: String,
    pub chunks: Vec<ChunkPreview>,
    pub budget: BudgetBreakdown,
    pub payments: Vec<PaymentLine>,
    pub formatted_context: String,
    pub total_tokens: u32,
}

#[derive(Deserialize, Debug)]
pub struct ChunkPreview {
    pub chunk_id: u64,
    pub article_id: String,
    pub relative_path: String,
    pub owner_address: String,
    pub title: String,
    pub heading_path: Vec<String>,
    pub chunk_token_count: u32,
    pub scores: ChunkScores,
    pub text: String,
    pub content_type: String,
    pub language: Option<String>,
    pub truncated: Option<TruncatedInfo>,
}

#[derive(Deserialize, Debug)]
pub struct ChunkScores {
    pub document: f32,
    pub passage: f32,
    pub blended: f32,
    pub vector: Option<f32>,
    pub lexical: Option<f32>,
}

// ── Shared ──

#[derive(Deserialize, Debug)]
pub struct BudgetBreakdown {
    pub paid_usdc: u64,
    pub consumed_usdc: u64,
    pub remaining_usdc: u64,
    pub platform_fee_usdc: u64,
    pub creators_fee_usdc: u64,
    pub items_returned: u32,
    pub items_searched: u32,
}

#[derive(Deserialize, Debug)]
pub struct PaymentLine {
    pub recipient: String,
    pub amount_usdc: u64,
}

/// BCS-serializable message that the client signs
#[derive(Serialize, Deserialize)]
pub struct ApiAccessMessage {
    pub q: String,
    pub wiki: String,
    pub transaction_digest: String,
    pub timestamp: u64,
    pub expand: Option<bool>,
}

// ── Agent-focused output (for --json flag) ──

#[derive(Serialize)]
pub struct AgentSearchResult {
    pub article_id: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub heading_path: Vec<String>,
    pub score: f64,
    pub article_token_count: u32,
    pub recency_days: u32,
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct AgentSearchResponse {
    pub results: Vec<AgentSearchResult>,
    pub budget: AgentBudget,
}

#[derive(Serialize)]
pub struct AgentChunkResult {
    pub chunk_id: u64,
    pub article_id: String,
    pub title: String,
    pub text: String,
    pub score: f32,
    pub chunk_token_count: u32,
    pub heading_path: Vec<String>,
    pub content_type: String,
    pub language: Option<String>,
    pub truncated: Option<TruncatedInfo>,
}

#[derive(Serialize)]
pub struct AgentChunksResponse {
    pub chunks: Vec<AgentChunkResult>,
    pub budget: AgentBudget,
}

#[derive(Serialize)]
pub struct AgentBudget {
    pub paid_usdc: u64,
    pub consumed_usdc: u64,
    pub remaining_usdc: u64,
}

// ── Expand Request / Response ──

/// BCS-serializable message for the expand endpoint
#[derive(Serialize, Deserialize)]
pub struct ExpandAccessMessage {
    pub chunk_ids: Vec<i64>,
    pub transaction_digest: String,
    pub timestamp: u64,
}

#[derive(Serialize)]
pub struct ExpandRequest {
    pub chunk_ids: Vec<i64>,
    pub transaction_digest: String,
    pub signature: String,
    pub bytes: String,
}

#[derive(Deserialize, Debug)]
pub struct ExpandResponse {
    pub chunks: Vec<ExpandedChunk>,
    pub budget: BudgetBreakdown,
    pub payments: Vec<PaymentLine>,
}

#[derive(Deserialize, Debug)]
pub struct ExpandedChunk {
    pub chunk_id: u64,
    pub article_id: String,
    pub owner_address: String,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub char_start: u32,
    pub char_end: u32,
    pub token_count: u32,
    pub chunk_text: String,
    pub content_type: String,
    pub language: Option<String>,
    pub truncated: Option<TruncatedInfo>,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct TruncatedInfo {
    pub content_type: String,
    #[serde(default)]
    pub table_rows_total: Option<u32>,
    #[serde(default)]
    pub table_rows_shown: Option<u32>,
    #[serde(default)]
    pub code_lines_total: Option<u32>,
    #[serde(default)]
    pub code_lines_shown: Option<u32>,
    #[serde(default)]
    pub prose_chars_total: Option<u32>,
    #[serde(default)]
    pub prose_chars_shown: Option<u32>,
}

#[derive(Serialize)]
pub struct AgentExpandedChunk {
    pub chunk_id: u64,
    pub article_id: String,
    pub heading_path: Vec<String>,
    pub chunk_text: String,
    pub content_type: String,
    pub token_count: u32,
    pub truncated: Option<TruncatedInfo>,
}

#[derive(Serialize)]
pub struct AgentExpandResponse {
    pub chunks: Vec<AgentExpandedChunk>,
    pub budget: AgentBudget,
}
