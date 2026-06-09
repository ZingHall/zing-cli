use crate::types::*;
use anyhow::Context;

const LLM_SYSTEM_PROMPT: &str = r#"You are an expert evaluator of RAG (Retrieval-Augmented Generation) answer quality.
You will receive a question, source context, a generated answer, and a ground truth answer.
Your job is to score the generated answer on factual accuracy, completeness, and whether it hallucinates.

Scoring:
- factual_accuracy (1-5): How factually correct is the answer compared to the ground truth?
  1 = completely wrong, 3 = partially correct, 5 = perfectly correct
- completeness (1-5): How complete is the answer?
  1 = missing most key points, 3 = covers some but not all, 5 = covers all key points
- hallucination (true/false): Does the answer contain any claims NOT present in EITHER the source context OR the ground truth? Extra information not in the sources = hallucination.

Respond ONLY with valid JSON matching this schema:
{
  "factual_accuracy": <1-5>,
  "completeness": <1-5>,
  "hallucination": <true|false>,
  "comment": "<brief explanation>"
}"#;

pub struct LlmJudge {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl LlmJudge {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        }
    }

    pub async fn evaluate(
        &self,
        query: &GoldenQuery,
        answer: &str,
    ) -> anyhow::Result<(QueryL3Result, Vec<FailureEntry>)> {
        let ground_truth = query.ground_truth_answer.as_deref().unwrap_or("(no ground truth provided)");
        let question_text = &query.query;

        let user_prompt = format!(
            "Question: {question}\n\nGround truth answer: {ground_truth}\n\nGenerated answer to evaluate: {answer}",
            question = question_text,
            ground_truth = ground_truth,
            answer = answer,
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": LLM_SYSTEM_PROMPT},
                {"role": "user", "content": user_prompt},
            ],
            "temperature": 0.0,
            "max_tokens": 300,
        });

        let resp = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .with_context(|| "LLM API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status.as_u16(), body_text);
        }

        #[derive(serde::Deserialize)]
        struct LlmResponse {
            choices: Vec<Choice>,
        }
        #[derive(serde::Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(serde::Deserialize)]
        struct Message {
            content: String,
        }

        let llm_resp: LlmResponse = resp.json().await.with_context(|| "Failed to parse LLM response")?;

        let raw_content = llm_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        #[derive(serde::Deserialize)]
        struct JudgeOutput {
            factual_accuracy: f64,
            completeness: f64,
            hallucination: bool,
            comment: String,
        }

        let stripped_content = if let Some(code) = raw_content
            .trim()
            .strip_prefix("```json")
            .and_then(|s| s.strip_suffix("```"))
        {
            code.trim().to_string()
        } else {
            raw_content.clone()
        };

        let judge: JudgeOutput = serde_json::from_str(&stripped_content)
            .with_context(|| format!("Failed to parse judge output: {}", raw_content))?;

        let mut failures = Vec::new();

        if judge.factual_accuracy < 3.0 {
            failures.push(FailureEntry {
                query_id: query.id.clone(),
                category: FailureCategory::RetrievalFailure,
                detail: format!("factual_accuracy={:.1}: {}", judge.factual_accuracy, judge.comment),
            });
        }

        if judge.hallucination {
            failures.push(FailureEntry {
                query_id: query.id.clone(),
                category: FailureCategory::Hallucination,
                detail: judge.comment.clone(),
            });
        }

        if judge.completeness < 3.0 {
            failures.push(FailureEntry {
                query_id: query.id.clone(),
                category: FailureCategory::ContextFailure,
                detail: format!("completeness={:.1}", judge.completeness),
            });
        }

        let result = QueryL3Result {
            query_id: query.id.clone(),
            factual_accuracy: judge.factual_accuracy,
            completeness: judge.completeness,
            hallucination: judge.hallucination,
            judge_comment: judge.comment,
        };

        Ok((result, failures))
    }
}
