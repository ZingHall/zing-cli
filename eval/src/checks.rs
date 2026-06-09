use crate::types::*;
use colored::Colorize;

const TOP_K: usize = 10;
const LARGE_STALENESS_DAYS: u32 = 365;

// ── Level 1: Per-Query Retrieval Quality ──

pub fn evaluate_query_chunks(
    query: &GoldenQuery,
    response: &EstimateChunksResponse,
) -> QueryL1Result {
    let mut checks = Vec::new();
    let mut best_rank: Option<usize> = None;
    let mut best_blended_score: Option<f32> = None;
    let mut best_heading_path: Option<Vec<String>> = None;
    let mut best_text_snippet: Option<String> = None;
    let mut not_expected_violations: Vec<String> = Vec::new();

    // Find the best-matching expected chunk across all results
    for (i, expected) in query.expected_chunks.iter().enumerate() {
        let label = format!("expected[{}]", i + 1);
        let mut found_any = false;
        let mut best_match_rank: Option<usize> = None;
        let mut best_match_score: f32 = 0.0;

        for (rank, chunk) in response.chunks.iter().enumerate() {
            let heading_text = chunk.heading_path.join(" > ");
            let mut matches = true;

            // Check heading_path_contains
            for pattern in &expected.heading_path_contains {
                if !heading_text.to_lowercase().contains(&pattern.to_lowercase()) {
                    matches = false;
                    break;
                }
            }

            // Check text_contains
            if let Some(ref text_pat) = expected.text_contains {
                if !chunk.text.to_lowercase().contains(&text_pat.to_lowercase()) {
                    matches = false;
                }
            }

            // Check text_contains_any (OR semantics)
            if !expected.text_contains_any.is_empty() {
                let any_match = expected
                    .text_contains_any
                    .iter()
                    .any(|pat| chunk.text.to_lowercase().contains(&pat.to_lowercase()));
                if !any_match {
                    matches = false;
                }
            }

            if matches {
                found_any = true;
                if best_match_rank.is_none() || chunk.scores.blended > best_match_score {
                    best_match_rank = Some(rank + 1);
                    best_match_score = chunk.scores.blended;
                }
                break; // first match is sufficient for "found"
            }
        }

        if found_any {
            let rank_ok = best_match_rank.unwrap() <= expected.max_rank;
            let score_ok = match expected.min_blended_score {
                Some(min) => best_match_score >= min,
                None => true,
            };

            if rank_ok && score_ok {
                checks.push(PerCheckResult {
                    check_name: format!("{}_match", label),
                    status: "PASS".green().to_string(),
                    detail: format!(
                        "rank={}, blended={:.3}",
                        best_match_rank.unwrap(),
                        best_match_score
                    ),
                });
            } else {
                let reasons: Vec<&str> = {
                    let mut r = Vec::new();
                    if !rank_ok {
                        r.push("rank too low");
                    }
                    if !score_ok {
                        r.push("score below threshold");
                    }
                    r
                };
                checks.push(PerCheckResult {
                    check_name: format!("{}_match", label),
                    status: "FAIL".red().to_string(),
                    detail: format!(
                        "rank={}, blended={:.3}, reasons=[{}]",
                        best_match_rank.unwrap(),
                        best_match_score,
                        reasons.join(", ")
                    ),
                });
            }

            if best_rank.is_none() || best_match_rank.unwrap() < best_rank.unwrap() {
                best_rank = best_match_rank;
                best_blended_score = Some(best_match_score);
            }
        } else {
            checks.push(PerCheckResult {
                check_name: format!("{}_match", label),
                status: "FAIL".red().to_string(),
                detail: "expected chunk not found in results".to_string(),
            });
        }
    }

    // Update best heading/text from the best-ranked result
    if let Some(rank) = best_rank {
        let idx = rank - 1;
        if let Some(chunk) = response.chunks.get(idx) {
            best_heading_path = Some(chunk.heading_path.clone());
            let snippet = if chunk.text.len() > 200 {
                &chunk.text[..200]
            } else {
                &chunk.text
            };
            best_text_snippet = Some(snippet.to_string());
        }
    }

    // Check not_expected conditions (top-K, only non-matching chunks)
    if !query.not_expected.is_empty() {
        for chunk in response.chunks.iter().take(TOP_K) {
            let heading_text = chunk.heading_path.join(" > ");
            let matches_any_expected = !query.expected_chunks.is_empty()
                && query.expected_chunks.iter().any(|exp| {
                    let h_match = exp
                        .heading_path_contains
                        .iter()
                        .all(|p| heading_text.to_lowercase().contains(&p.to_lowercase()));
                    let t_match = exp
                        .text_contains
                        .as_ref()
                        .map(|t| chunk.text.to_lowercase().contains(&t.to_lowercase()))
                        .unwrap_or(true);
                    let ta_match = if exp.text_contains_any.is_empty() {
                        true
                    } else {
                        exp.text_contains_any
                            .iter()
                            .any(|pat| chunk.text.to_lowercase().contains(&pat.to_lowercase()))
                    };
                    h_match && t_match && ta_match
                });
            if matches_any_expected {
                continue;
            }

            for not_exp in &query.not_expected {
                if let Some(ref h) = not_exp.heading_contains {
                    if heading_text.to_lowercase().contains(&h.to_lowercase()) {
                        not_expected_violations.push(format!(
                            "heading contains forbidden text: \"{}\" at rank",
                            h
                        ));
                    }
                }
                if let Some(ref t) = not_exp.text_contains {
                    if chunk.text.to_lowercase().contains(&t.to_lowercase()) {
                        not_expected_violations.push(format!(
                            "text contains forbidden text: \"{}\"",
                            t
                        ));
                    }
                }
            }
        }
    }

    if not_expected_violations.is_empty() {
        checks.push(PerCheckResult {
            check_name: "not_expected".to_string(),
            status: "PASS".green().to_string(),
            detail: "no forbidden content found in top-K non-matching results".to_string(),
        });
    } else {
        checks.push(PerCheckResult {
            check_name: "not_expected".to_string(),
            status: "WARN".yellow().to_string(),
            detail: not_expected_violations.join("; "),
        });
    }

    // Recall in top-K
    let recall = query.expected_chunks.iter().fold(0usize, |acc, expected| {
        let found = response.chunks.iter().take(TOP_K).any(|chunk| {
            let heading_text = chunk.heading_path.join(" > ");
            let mut matches = true;
            for p in &expected.heading_path_contains {
                if !heading_text.to_lowercase().contains(&p.to_lowercase()) {
                    matches = false;
                    break;
                }
            }
            if let Some(ref t) = expected.text_contains {
                if !chunk.text.to_lowercase().contains(&t.to_lowercase()) {
                    matches = false;
                }
            }
            if !expected.text_contains_any.is_empty() {
                let any = expected.text_contains_any.iter().any(|pat| {
                    chunk.text.to_lowercase().contains(&pat.to_lowercase())
                });
                if !any {
                    matches = false;
                }
            }
            matches
        });
        if found { acc + 1 } else { acc }
    });

    let recall_total = query.expected_chunks.len();
    if recall_total > 0 {
        let recall_frac = recall as f64 / recall_total as f64;
        let status = if recall_frac >= 0.8 {
            "PASS".green().to_string()
        } else {
            "FAIL".red().to_string()
        };
        checks.push(PerCheckResult {
            check_name: "recall_in_top_k".to_string(),
            status,
            detail: format!("{}/{} expected chunks in top-{}", recall, recall_total, TOP_K),
        });
    }

    let total_fails = checks.iter().filter(|c| c.status.contains("FAIL")).count();
    let overall_status = if total_fails == 0 { "PASS" } else { "FAIL" };

    QueryL1Result {
        query_id: query.id.clone(),
        endpoint: format!("{:?}", query.endpoint),
        status: overall_status.to_string(),
        num_results: response.chunks.len(),
        expected_count: query.expected_chunks.len(),
        checks,
        details: QueryL1Details {
            best_rank,
            best_blended_score,
            best_heading_path,
            best_text_snippet,
            best_relevance_score: None,
            not_expected_violations: if not_expected_violations.is_empty() {
                None
            } else {
                Some(not_expected_violations)
            },
        },
    }
}

pub fn evaluate_query_search(
    query: &GoldenQuery,
    response: &EstimateSearchResponse,
) -> QueryL1Result {
    let mut checks = Vec::new();
    let mut best_rank: Option<usize> = None;
    let mut best_relevance_score: Option<f64> = None;
    let mut best_heading_path: Option<Vec<String>> = None;
    let mut best_text_snippet: Option<String> = None;
    let mut not_expected_violations: Vec<String> = Vec::new();

    // Find best-matching expected chunk in search results
    for (i, expected) in query.expected_chunks.iter().enumerate() {
        let label = format!("expected[{}]", i + 1);
        let mut found_any = false;
        let mut best_match_rank: Option<usize> = None;
        let mut best_match_score: f64 = 0.0;

        for (rank, result) in response.results.iter().enumerate() {
            let heading_text = result
                .best_match
                .as_ref()
                .map(|m| m.heading_path.join(" > "))
                .unwrap_or_default();
            let text = result
                .best_match
                .as_ref()
                .map(|m| m.excerpt.as_str())
                .unwrap_or("");

            let mut matches = true;
            for pattern in &expected.heading_path_contains {
                if !heading_text.to_lowercase().contains(&pattern.to_lowercase()) {
                    matches = false;
                    break;
                }
            }
            if let Some(ref text_pat) = expected.text_contains {
                if !text.to_lowercase().contains(&text_pat.to_lowercase()) {
                    matches = false;
                }
            }
            if !expected.text_contains_any.is_empty() {
                let any_match = expected
                    .text_contains_any
                    .iter()
                    .any(|pat| text.to_lowercase().contains(&pat.to_lowercase()));
                if !any_match {
                    matches = false;
                }
            }
            if matches {
                found_any = true;
                if best_match_rank.is_none()
                    || result.signals.relevance_score > best_match_score
                {
                    best_match_rank = Some(rank + 1);
                    best_match_score = result.signals.relevance_score;
                }
                break;
            }
        }

        if found_any {
            let rank_ok = best_match_rank.unwrap() <= expected.max_rank;
            if rank_ok {
                checks.push(PerCheckResult {
                    check_name: format!("{}_match", label),
                    status: "PASS".green().to_string(),
                    detail: format!(
                        "rank={}, relevance={:.3}",
                        best_match_rank.unwrap(),
                        best_match_score
                    ),
                });
            } else {
                checks.push(PerCheckResult {
                    check_name: format!("{}_match", label),
                    status: "FAIL".red().to_string(),
                    detail: format!(
                        "found at rank={} but max_rank={}",
                        best_match_rank.unwrap(),
                        expected.max_rank
                    ),
                });
            }
            if best_rank.is_none() || best_match_rank.unwrap() < best_rank.unwrap() {
                best_rank = best_match_rank;
                best_relevance_score = Some(best_match_score);
            }
        } else {
            checks.push(PerCheckResult {
                check_name: format!("{}_match", label),
                status: "FAIL".red().to_string(),
                detail: "expected not found in search results".to_string(),
            });
        }
    }

    if let Some(rank) = best_rank {
        let idx = rank - 1;
        if let Some(result) = response.results.get(idx) {
            best_heading_path = result
                .best_match
                .as_ref()
                .map(|m| m.heading_path.clone());
            best_text_snippet = result
                .best_match
                .as_ref()
                .map(|m| if m.excerpt.len() > 200 { &m.excerpt[..200] } else { &m.excerpt })
                .map(|s| s.to_string());
        }
    }

    // not_expected (top-K, only non-matching results)
    if !query.not_expected.is_empty() {
        for result in response.results.iter().take(TOP_K) {
            let heading_text = result
                .best_match
                .as_ref()
                .map(|m| m.heading_path.join(" > "))
                .unwrap_or_default();
            let text = result
                .best_match
                .as_ref()
                .map(|m| m.excerpt.as_str())
                .unwrap_or("");

            let matches_any_expected = !query.expected_chunks.is_empty()
                && query.expected_chunks.iter().any(|exp| {
                    let h_match = exp
                        .heading_path_contains
                        .iter()
                        .all(|p| heading_text.to_lowercase().contains(&p.to_lowercase()));
                    let t_match = exp
                        .text_contains
                        .as_ref()
                        .map(|t| text.to_lowercase().contains(&t.to_lowercase()))
                        .unwrap_or(true);
                    let ta_match = if exp.text_contains_any.is_empty() {
                        true
                    } else {
                        exp.text_contains_any
                            .iter()
                            .any(|pat| text.to_lowercase().contains(&pat.to_lowercase()))
                    };
                    h_match && t_match && ta_match
                });
            if matches_any_expected {
                continue;
            }

            for not_exp in &query.not_expected {
                if let Some(ref h) = not_exp.heading_contains {
                    if heading_text.to_lowercase().contains(&h.to_lowercase()) {
                        not_expected_violations
                            .push(format!("heading contains forbidden: \"{}\"", h));
                    }
                }
                if let Some(ref t) = not_exp.text_contains {
                    if text.to_lowercase().contains(&t.to_lowercase()) {
                        not_expected_violations
                            .push(format!("text contains forbidden: \"{}\"", t));
                    }
                }
            }
        }
    }

    if not_expected_violations.is_empty() {
        checks.push(PerCheckResult {
            check_name: "not_expected".to_string(),
            status: "PASS".green().to_string(),
            detail: "no forbidden content found".to_string(),
        });
    } else {
        checks.push(PerCheckResult {
            check_name: "not_expected".to_string(),
            status: "WARN".yellow().to_string(),
            detail: not_expected_violations.join("; "),
        });
    }

    // Recall in top-K
    let recall = query
        .expected_chunks
        .iter()
        .fold(0usize, |acc, expected| {
            let found = response.results.iter().take(TOP_K).any(|result| {
                let heading_text = result
                    .best_match
                    .as_ref()
                    .map(|m| m.heading_path.join(" > "))
                    .unwrap_or_default();
                let text = result
                    .best_match
                    .as_ref()
                    .map(|m| m.excerpt.as_str())
                    .unwrap_or("");
                let mut matches = true;
                for p in &expected.heading_path_contains {
                    if !heading_text.to_lowercase().contains(&p.to_lowercase()) {
                        matches = false;
                        break;
                    }
                }
                if let Some(ref t) = expected.text_contains {
                    if !text.to_lowercase().contains(&t.to_lowercase()) {
                        matches = false;
                    }
                }
                if !expected.text_contains_any.is_empty() {
                    let any = expected.text_contains_any.iter().any(|pat| {
                        text.to_lowercase().contains(&pat.to_lowercase())
                    });
                    if !any {
                        matches = false;
                    }
                }
                matches
            });
            if found { acc + 1 } else { acc }
        });

    let recall_total = query.expected_chunks.len();
    if recall_total > 0 {
        let recall_frac = recall as f64 / recall_total as f64;
        let status = if recall_frac >= 0.8 {
            "PASS".green().to_string()
        } else {
            "FAIL".red().to_string()
        };
        checks.push(PerCheckResult {
            check_name: "recall_in_top_k".to_string(),
            status,
            detail: format!("{}/{} expected in top-{}", recall, recall_total, TOP_K),
        });
    }

    let total_fails = checks.iter().filter(|c| c.status.contains("FAIL")).count();
    let overall_status = if total_fails == 0 { "PASS" } else { "FAIL" };

    QueryL1Result {
        query_id: query.id.clone(),
        endpoint: format!("{:?}", query.endpoint),
        status: overall_status.to_string(),
        num_results: response.results.len(),
        expected_count: query.expected_chunks.len(),
        checks,
        details: QueryL1Details {
            best_rank,
            best_blended_score: None,
            best_heading_path,
            best_text_snippet,
            best_relevance_score,
            not_expected_violations: if not_expected_violations.is_empty() {
                None
            } else {
                Some(not_expected_violations)
            },
        },
    }
}

// ── Level 2: Score Sanity ──

pub fn check_score_sanity_chunks(
    query: &GoldenQuery,
    response: &EstimateChunksResponse,
    expected: &[ExpectedChunk],
) -> Vec<Level2Flag> {
    let mut flags = Vec::new();
    let scores = ScoreCollection::from_chunks(&response.chunks);

    // 1. Irrelevant high scores
    for chunk in &response.chunks {
        if chunk.scores.blended > 0.9 {
            let relevant = expected.iter().any(|exp| {
                let heading_text = chunk.heading_path.join(" > ");
                let h_ok = exp
                    .heading_path_contains
                    .iter()
                    .all(|p| heading_text.to_lowercase().contains(&p.to_lowercase()));
                let t_ok = exp
                    .text_contains
                    .as_ref()
                    .map(|t| chunk.text.to_lowercase().contains(&t.to_lowercase()))
                    .unwrap_or(true);
                let ta_ok = if exp.text_contains_any.is_empty() {
                    true
                } else {
                    exp.text_contains_any.iter().any(|pat| {
                        chunk.text.to_lowercase().contains(&pat.to_lowercase())
                    })
                };
                h_ok && t_ok && ta_ok
            });
            if !relevant {
                flags.push(Level2Flag {
                    query_id: query.id.clone(),
                    flag_type: "irrelevant_high_score".to_string(),
                    detail: format!(
                        "chunk_id={} blended={:.3} heading={}",
                        chunk.chunk_id,
                        chunk.scores.blended,
                        chunk.heading_path.join(" > ")
                    ),
                });
            }
        }
    }

    // 2. Score clustering (IQR < 0.05)
    if !scores.blended_scores.is_empty() {
        let iqr = scores.iqr(&scores.blended_scores);
        if iqr < 0.05 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "score_clustering".to_string(),
                detail: format!(
                    "IQR={:.3}, all {} results clustered tight",
                    iqr,
                    scores.blended_scores.len()
                ),
            });
        }
    }

    // 3. Code chunk suppression
    let code_scores: Vec<f32> = response
        .chunks
        .iter()
        .filter(|c| c.content_type == "code")
        .map(|c| c.scores.blended)
        .collect();
    if !code_scores.is_empty() {
        let avg_code = code_scores.iter().sum::<f32>() / code_scores.len() as f32;
        if avg_code < 0.2 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "code_chunk_suppression".to_string(),
                detail: format!("avg code blended={:.3} across {} code chunks", avg_code, code_scores.len()),
            });
        }
    }

    // 4. Recall failure (expected not in top-10)
    for exp in expected {
        let found_in_top10 = response.chunks.iter().take(10).any(|chunk| {
            let heading_text = chunk.heading_path.join(" > ");
            let h_match = exp
                .heading_path_contains
                .iter()
                .all(|p| heading_text.to_lowercase().contains(&p.to_lowercase()));
            let t_match = exp
                .text_contains
                .as_ref()
                .map(|t| chunk.text.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(true);
            let ta_match = if exp.text_contains_any.is_empty() {
                true
            } else {
                exp.text_contains_any.iter().any(|pat| {
                    chunk.text.to_lowercase().contains(&pat.to_lowercase())
                })
            };
            h_match && t_match && ta_match
        });
        if !found_in_top10 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "recall_failure_top10".to_string(),
                detail: format!(
                    "expected chunk not in top-10 (heading_path_contains={:?})",
                    exp.heading_path_contains
                ),
            });
        }
    }

    // 5. Null lexical scores
    let total = scores.lexical_scores.len();
    let null_lexical = scores.lexical_scores.iter().filter(|s| s.is_none()).count();
    if total > 0 && (null_lexical as f64 / total as f64) > 0.8 {
        flags.push(Level2Flag {
            query_id: query.id.clone(),
            flag_type: "null_lexical_scores".to_string(),
            detail: format!("lexical is null on {}/{} results", null_lexical, total),
        });
    }

    // 6. Vector/lexical imbalance
    let vec_mean: f64 = scores
        .vector_scores
        .iter()
        .filter_map(|v| v.map(|x| x as f64))
        .sum::<f64>();
    let lex_mean: f64 = scores
        .lexical_scores
        .iter()
        .filter_map(|v| v.map(|x| x as f64))
        .sum::<f64>();
    let vec_count = scores.vector_scores.iter().filter(|v| v.is_some()).count() as f64;
    let lex_count = scores.lexical_scores.iter().filter(|v| v.is_some()).count() as f64;

    if vec_count > 0.0 && lex_count > 0.0 {
        let avg_vec = vec_mean / vec_count;
        let avg_lex = lex_mean / lex_count;
        if avg_vec > avg_lex * 3.0 || avg_lex > avg_vec * 3.0 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "score_imbalance".to_string(),
                detail: format!(
                    "avg_vector={:.3} vs avg_lexical={:.3} (ratio > 3x)",
                    avg_vec, avg_lex
                ),
            });
        }
    }

    flags
}

pub fn check_score_sanity_search(
    query: &GoldenQuery,
    response: &EstimateSearchResponse,
) -> Vec<Level2Flag> {
    let mut flags = Vec::new();
    let scores = ScoreCollection::from_search(&response.results);

    // 1. Irrelevant high scores
    for result in &response.results {
        if result.signals.relevance_score > 0.9 {
            let heading_text = result.best_match.as_ref()
                .map(|m| m.heading_path.join(" > "))
                .unwrap_or_default();
            let relevant = query.expected_chunks.iter().any(|exp| {
                exp.heading_path_contains.iter().all(|p| {
                    heading_text.to_lowercase().contains(&p.to_lowercase())
                })
            });
            if !relevant && !query.expected_chunks.is_empty() {
                flags.push(Level2Flag {
                    query_id: query.id.clone(),
                    flag_type: "irrelevant_high_score".to_string(),
                    detail: format!(
                        "relevance={:.3} heading={} not in expected",
                        result.signals.relevance_score, heading_text
                    ),
                });
            }
        }
    }

    // 2. Score clustering on relevance_score
    if !scores.relevance_scores.is_empty() {
        let iqr = calculate_f64_iqr(&scores.relevance_scores);
        if iqr < 0.05 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "score_clustering".to_string(),
                detail: format!(
                    "IQR={:.3} on relevance_scores, {} results",
                    iqr,
                    scores.relevance_scores.len()
                ),
            });
        }
    }

    // 3. Null lexical scores
    let total = scores.lexical_scores.len();
    let null_lex = scores.lexical_scores.iter().filter(|s| s.is_none()).count();
    if total > 0 && (null_lex as f64 / total as f64) > 0.8 {
        flags.push(Level2Flag {
            query_id: query.id.clone(),
            flag_type: "null_lexical_scores".to_string(),
            detail: format!("lexical null on {}/{} results", null_lex, total),
        });
    }

    // 4. Stale content penalty check
    if !scores.relevance_scores.is_empty() && !scores.recency_days.is_empty() {
        let high_recency: Vec<&u32> = scores
            .recency_days
            .iter()
            .filter(|&&r| r > LARGE_STALENESS_DAYS)
            .collect();
        if !high_recency.is_empty() {
            let stale_pct = high_recency.len() as f64 / scores.recency_days.len() as f64 * 100.0;
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "stale_content_dominance".to_string(),
                detail: format!(
                    "{:.0}% of results > {} days old",
                    stale_pct, LARGE_STALENESS_DAYS
                ),
            });
        }
    }

    // 5. Score imbalance
    let vec_mean = scores.vector_scores.iter().filter_map(|v| v.map(|x| x as f64)).sum::<f64>();
    let lex_mean = scores.lexical_scores.iter().filter_map(|v| v.map(|x| x as f64)).sum::<f64>();
    let vec_count = scores.vector_scores.iter().filter(|v| v.is_some()).count() as f64;
    let lex_count = scores.lexical_scores.iter().filter(|v| v.is_some()).count() as f64;
    if vec_count > 0.0 && lex_count > 0.0 {
        let avg_vec = vec_mean / vec_count;
        let avg_lex = lex_mean / lex_count;
        if avg_vec > avg_lex * 3.0 || avg_lex > avg_vec * 3.0 {
            flags.push(Level2Flag {
                query_id: query.id.clone(),
                flag_type: "score_imbalance".to_string(),
                detail: format!("avg_vec={:.3} vs avg_lex={:.3} ratio >3x", avg_vec, avg_lex),
            });
        }
    }

    flags
}

fn calculate_f64_iqr(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let q1 = sorted[(n as f64 * 0.25) as usize];
    let q3 = sorted[(n as f64 * 0.75) as usize];
    q3 - q1
}
