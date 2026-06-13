use serde::{Deserialize, Serialize};

// ── Golden Query Definitions ──

#[derive(Deserialize, Debug, Clone)]
pub struct GoldenQuery {
    pub id: String,
    pub endpoint: Endpoint,
    pub query: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub question_type: QuestionType,
    #[serde(default)]
    pub expected_chunks: Vec<ExpectedChunk>,
    #[serde(default)]
    pub not_expected: Vec<NotExpected>,
    #[serde(default)]
    pub ground_truth_answer: Option<String>,
    #[serde(default)]
    pub eval_note: Option<String>,
}

fn default_limit() -> u32 {
    20
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Endpoint {
    Chunks,
    Search,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuestionType {
    Factual,
    Procedural,
    Conceptual,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExpectedChunk {
    #[serde(default)]
    pub heading_path_contains: Vec<String>,
    #[serde(default)]
    pub text_contains: Option<String>,
    #[serde(default)]
    pub text_contains_any: Vec<String>,
    #[serde(default)]
    pub min_blended_score: Option<f32>,
    #[serde(default = "default_rank")]
    pub max_rank: usize,
}

fn default_rank() -> usize {
    10
}

#[derive(Deserialize, Debug, Clone)]
pub struct NotExpected {
    #[serde(default)]
    pub heading_contains: Option<String>,
    #[serde(default)]
    pub text_contains: Option<String>,
}

// ── API Response Types ──

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct EstimateChunksResponse {
    pub chunks: Vec<EstimateChunk>,
    pub budget: EstimateBudget,
    #[serde(default)]
    pub total_tokens: u32,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct EstimateChunk {
    pub chunk_id: u64,
    #[serde(default)]
    pub article_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub heading_path: Vec<String>,
    #[serde(default)]
    pub chunk_token_count: u32,
    pub scores: ChunkScores,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub truncated: Option<TruncatedInfo>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct ChunkScores {
    pub document: f32,
    pub passage: f32,
    pub blended: f32,
    #[serde(default)]
    pub vector: Option<f32>,
    #[serde(default)]
    pub lexical: Option<f32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct EstimateSearchResponse {
    pub results: Vec<EstimateSearchResult>,
    pub budget: EstimateBudget,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct EstimateSearchResult {
    pub article_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub best_match: Option<BestMatch>,
    #[serde(default)]
    pub chunk_token_count: u32,
    #[serde(default)]
    pub raw_vector_score: Option<f64>,
    #[serde(default)]
    pub raw_lexical_score: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub signals: Signals,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct BestMatch {
    pub excerpt: String,
    #[serde(default)]
    pub heading_path: Vec<String>,
    pub char_start: u32,
    pub char_end: u32,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct Signals {
    pub relevance_score: f64,
    pub article_token_count: u32,
    pub recency_days: u32,
    pub tag_confidence: f64,
    pub wiki_file_count: u32,
    pub primary_tag: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct EstimateBudget {
    pub paid_usdc: String,
    pub consumed_usdc: String,
    pub remaining_usdc: String,
}

// ── Evaluation Result Types ──

#[derive(Serialize, Debug, Clone)]
pub struct EvalRun {
    pub run_id: String,
    pub timestamp: String,
    pub api_url: String,
    pub total_queries: usize,
    pub l1_results: Option<Level1Results>,
    pub l2_flags: Option<Vec<Level2Flag>>,
    pub l3_results: Option<Level3Results>,
    pub failures: Vec<FailureEntry>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Level1Results {
    pub passed: usize,
    pub failed: usize,
    pub queries: Vec<QueryL1Result>,
}

#[derive(Serialize, Debug, Clone)]
pub struct QueryL1Result {
    pub query_id: String,
    pub endpoint: String,
    pub status: String,
    pub num_results: usize,
    pub expected_count: usize,
    pub checks: Vec<PerCheckResult>,
    pub details: QueryL1Details,
}

#[derive(Serialize, Debug, Clone)]
pub struct PerCheckResult {
    pub check_name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct QueryL1Details {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_rank: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_blended_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_heading_path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_text_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_relevance_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_expected_violations: Option<Vec<String>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Level2Flag {
    pub query_id: String,
    pub flag_type: String,
    pub detail: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Level3Results {
    pub avg_factual_accuracy: f64,
    pub avg_completeness: f64,
    pub hallucination_count: usize,
    pub queries: Vec<QueryL3Result>,
}

#[derive(Serialize, Debug, Clone)]
pub struct QueryL3Result {
    pub query_id: String,
    pub factual_accuracy: f64,
    pub completeness: f64,
    pub hallucination: bool,
    pub judge_comment: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct FailureEntry {
    pub query_id: String,
    pub category: FailureCategory,
    pub detail: String,
}

#[allow(dead_code)]
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    RetrievalFailure,
    RankingFailure,
    ContextFailure,
    Hallucination,
    Truncation,
}

// ── Score Collection for L2 ──

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ScoreCollection {
    pub blended_scores: Vec<f32>,
    pub vector_scores: Vec<Option<f32>>,
    pub lexical_scores: Vec<Option<f32>>,
    pub relevance_scores: Vec<f64>,
    pub content_types: Vec<String>,
    pub recency_days: Vec<u32>,
}

impl ScoreCollection {
    pub fn from_chunks(chunks: &[EstimateChunk]) -> Self {
        Self {
            blended_scores: chunks.iter().map(|c| c.scores.blended).collect(),
            vector_scores: chunks.iter().map(|c| c.scores.vector).collect(),
            lexical_scores: chunks.iter().map(|c| c.scores.lexical).collect(),
            relevance_scores: vec![],
            content_types: chunks.iter().map(|c| c.content_type.clone()).collect(),
            recency_days: vec![],
        }
    }

    pub fn from_search(results: &[EstimateSearchResult]) -> Self {
        Self {
            blended_scores: vec![],
            vector_scores: results.iter().map(|r| r.raw_vector_score.map(|v| v as f32)).collect(),
            lexical_scores: results.iter().map(|r| r.raw_lexical_score.map(|v| v as f32)).collect(),
            relevance_scores: results.iter().map(|r| r.signals.relevance_score).collect(),
            content_types: vec![],
            recency_days: results.iter().map(|r| r.signals.recency_days).collect(),
        }
    }

    pub fn iqr(&self, values: &[f32]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = values.iter().map(|&v| v as f64).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let q1 = sorted[(n as f64 * 0.25) as usize];
        let q3 = sorted[(n as f64 * 0.75) as usize];
        q3 - q1
    }
}
