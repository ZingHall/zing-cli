use crate::types::*;
use anyhow::Context;

pub struct ApiRunner {
    client: reqwest::Client,
    base_url: String,
    app_key: String,
}

impl ApiRunner {
    pub fn new(base_url: String, app_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            app_key,
        }
    }

    pub async fn chunk_estimate(
        &self,
        query: &GoldenQuery,
    ) -> anyhow::Result<EstimateChunksResponse> {
        let url = format!("{}/chunk/estimate", self.base_url);
        let body = serde_json::json!({
            "q": query.query,
            "wiki": query.owner.as_deref().unwrap_or("global"),
            "owner": query.owner,
            "limit": query.limit,
            "expand": true,
        });

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-App-Key", &self.app_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("HTTP request failed to {}", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body_text);
        }

        let data: EstimateChunksResponse = resp
            .json()
            .await
            .with_context(|| "Failed to parse chunk estimate response")?;

        Ok(data)
    }

    pub async fn search_estimate(
        &self,
        query: &GoldenQuery,
    ) -> anyhow::Result<EstimateSearchResponse> {
        let url = format!("{}/search/estimate", self.base_url);
        let body = serde_json::json!({
            "q": query.query,
            "wiki": query.owner.as_deref().unwrap_or("global"),
            "owner": query.owner,
            "limit": query.limit,
        });

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-App-Key", &self.app_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("HTTP request failed to {}", url))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body_text);
        }

        let data: EstimateSearchResponse = resp
            .json()
            .await
            .with_context(|| "Failed to parse search estimate response")?;

        Ok(data)
    }
}
