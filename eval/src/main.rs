mod checks;
mod golden;
mod l3_eval;
mod report;
mod runner;
mod types;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use types::*;

#[derive(Parser)]
#[command(
    name = "zing-eval",
    about = "RAG evaluation framework for the Zing search pipeline"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run evaluation tests
    Run {
        /// Comma-separated levels to run (1,2,3). Default: 1,2
        #[arg(long, default_value = "1,2")]
        levels: String,

        /// API base URL (default: $ZING_EVAL_API_URL or http://localhost:3001)
        #[arg(long)]
        api: Option<String>,

        /// App key for estimate endpoints (default: $ZING_EVAL_APP_KEY)
        #[arg(long)]
        app_key: Option<String>,

        /// LLM API key for Level 3 (default: $ZING_EVAL_LLM_KEY)
        #[arg(long)]
        llm_key: Option<String>,

        /// Query directory (default: $ZING_EVAL_QUERY_DIR or ./eval/queries)
        #[arg(long)]
        query_dir: Option<String>,

        /// Output directory for reports (default: ./eval/reports)
        #[arg(long, default_value = "./eval/reports")]
        output_dir: String,
    },
    /// Show scoring formulas from the API
    Formula {},
    /// List all golden queries and their descriptions
    List {
        #[arg(long)]
        query_dir: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            levels,
            api,
            app_key,
            llm_key,
            query_dir,
            output_dir,
        } => {
            run_eval(levels, api, app_key, llm_key, query_dir, output_dir).await?;
        }
        Command::Formula {} => {
            print_formulas().await?;
        }
        Command::List { query_dir } => {
            list_queries(query_dir)?;
        }
    }

    Ok(())
}

async fn run_eval(
    levels_str: String,
    api: Option<String>,
    app_key: Option<String>,
    llm_key: Option<String>,
    query_dir: Option<String>,
    output_dir: String,
) -> anyhow::Result<()> {
    let run_levels: Vec<u8> = levels_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let api_url = api
        .or_else(|| std::env::var("ZING_EVAL_API_URL").ok())
        .unwrap_or_else(|| "http://localhost:3001".to_string());
    let app_key = app_key
        .or_else(|| std::env::var("ZING_EVAL_APP_KEY").ok())
        .unwrap_or_else(|| {
            eprintln!(
                "{} Set ZING_EVAL_APP_KEY env var or pass --app-key",
                "WARNING:".yellow()
            );
            std::process::exit(1);
        });

    let default_query_dir = std::env::var("ZING_EVAL_QUERY_DIR")
        .unwrap_or_else(|_| "./eval/queries".to_string());
    let query_path = PathBuf::from(query_dir.unwrap_or(default_query_dir));

    let queries = golden::load_queries(&query_path)?;
    println!(
        "{} Loaded {} golden queries from {}",
        "INFO:".blue(),
        queries.len(),
        query_path.display()
    );

    let runner = runner::ApiRunner::new(api_url.clone(), app_key);

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let run_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();

    let mut l1_results: Option<Level1Results> = None;
    let mut l2_flags: Option<Vec<Level2Flag>> = None;
    let mut l3_results: Option<Level3Results> = None;
    let mut all_failures: Vec<FailureEntry> = Vec::new();

    // ── Level 1+2 ──
    if run_levels.contains(&1) || run_levels.contains(&2) {
        let mut l1_queries = Vec::new();
        let mut l1_passed = 0usize;
        let mut l1_failed = 0usize;
        let mut all_l2_flags: Vec<Level2Flag> = Vec::new();

        for query in &queries {
            println!("{} Running: {}", "▶".dimmed(), query.id);

            match query.endpoint {
                Endpoint::Chunks => {
                    match runner.chunk_estimate(query).await {
                        Ok(resp) => {
                            let l1 = checks::evaluate_query_chunks(query, &resp);
                            l1_queries.push(l1);

                            let l2 = checks::check_score_sanity_chunks(
                                query,
                                &resp,
                                &query.expected_chunks,
                            );
                            all_l2_flags.extend(l2);

                            // Check for truncation failures
                            for chunk in &resp.chunks {
                                if let Some(ref t) = chunk.truncated {
                                    all_failures.push(FailureEntry {
                                        query_id: query.id.clone(),
                                        category: FailureCategory::Truncation,
                                        detail: format!(
                                            "chunk_id={} content_type={} truncated",
                                            chunk.chunk_id, t.content_type
                                        ),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  {} API error for {}: {}", "✗".red(), query.id, e);
                            l1_queries.push(QueryL1Result {
                                query_id: query.id.clone(),
                                endpoint: "chunks".to_string(),
                                status: "ERROR".red().to_string(),
                                num_results: 0,
                                expected_count: query.expected_chunks.len(),
                                checks: vec![PerCheckResult {
                                    check_name: "api_error".to_string(),
                                    status: "ERROR".red().to_string(),
                                    detail: e.to_string(),
                                }],
                                details: QueryL1Details {
                                    best_rank: None,
                                    best_blended_score: None,
                                    best_heading_path: None,
                                    best_text_snippet: None,
                                    best_relevance_score: None,
                                    not_expected_violations: None,
                                },
                            });
                            l1_failed += 1;
                        }
                    }
                }
                Endpoint::Search => {
                    match runner.search_estimate(query).await {
                        Ok(resp) => {
                            let l1 = checks::evaluate_query_search(query, &resp);
                            l1_queries.push(l1);

                            let l2 =
                                checks::check_score_sanity_search(query, &resp);
                            all_l2_flags.extend(l2);
                        }
                        Err(e) => {
                            eprintln!("  {} API error for {}: {}", "✗".red(), query.id, e);
                            l1_queries.push(QueryL1Result {
                                query_id: query.id.clone(),
                                endpoint: "search".to_string(),
                                status: "ERROR".red().to_string(),
                                num_results: 0,
                                expected_count: query.expected_chunks.len(),
                                checks: vec![PerCheckResult {
                                    check_name: "api_error".to_string(),
                                    status: "ERROR".red().to_string(),
                                    detail: e.to_string(),
                                }],
                                details: QueryL1Details {
                                    best_rank: None,
                                    best_blended_score: None,
                                    best_heading_path: None,
                                    best_text_snippet: None,
                                    best_relevance_score: None,
                                    not_expected_violations: None,
                                },
                            });
                            l1_failed += 1;
                        }
                    }
                }
            }
        }

        for l1q in &l1_queries {
            if l1q.status == "PASS" {
                l1_passed += 1;
            } else {
                l1_failed += 1;
            }
        }

        if run_levels.contains(&1) {
            l1_results = Some(Level1Results {
                passed: l1_passed,
                failed: l1_failed,
                queries: l1_queries,
            });
        }
        if run_levels.contains(&2) {
            l2_flags = Some(all_l2_flags);
        }
    }

    // ── Level 3 ──
    if run_levels.contains(&3) {
        let llm_key = llm_key
            .or_else(|| std::env::var("ZING_EVAL_LLM_KEY").ok())
            .unwrap_or_else(|| {
                eprintln!(
                    "{} Level 3 requires ZING_EVAL_LLM_KEY env var or --llm-key",
                    "ERROR:".red()
                );
                std::process::exit(1);
            });

        let llm_model =
            std::env::var("ZING_EVAL_LLM_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
        let llm_base = std::env::var("ZING_EVAL_LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());

        let judge = l3_eval::LlmJudge::new(llm_key, llm_model, llm_base);
        let mut l3_queries = Vec::new();

        for query in &queries {
            if query.ground_truth_answer.is_none() {
                eprintln!(
                    "  {} Skipping {} — no ground_truth_answer",
                    "⚠".yellow(),
                    query.id
                );
                continue;
            }
            println!("{} L3 eval: {}", "▶".dimmed(), query.id);

            match runner.chunk_estimate(query).await {
                Ok(resp) => {
                    let context: String = resp
                        .chunks
                        .iter()
                        .enumerate()
                        .map(|(i, c)| format!("[Source {}] {}\n\"{}\"", i + 1, c.title, c.text))
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    let llm_answer = format!(
                        "Based on the retrieved context, the answer to '{}' is provided in the sources above.",
                        query.query
                    );

                    let full_answer = format!("{context}\n\n{llm_answer}", context = context, llm_answer = llm_answer);

                    match judge.evaluate(query, &full_answer).await {
                        Ok((l3_result, failures)) => {
                            l3_queries.push(l3_result);
                            all_failures.extend(failures);
                        }
                        Err(e) => {
                            eprintln!("  {} LLM eval failed for {}: {}", "✗".red(), query.id, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  {} API error for {}: {}", "✗".red(), query.id, e);
                }
            }
        }

        if !l3_queries.is_empty() {
            let n = l3_queries.len() as f64;
            let avg_accuracy = l3_queries.iter().map(|q| q.factual_accuracy).sum::<f64>() / n;
            let avg_completeness =
                l3_queries.iter().map(|q| q.completeness).sum::<f64>() / n;
            let hallucination_count = l3_queries.iter().filter(|q| q.hallucination).count();

            l3_results = Some(Level3Results {
                avg_factual_accuracy: avg_accuracy,
                avg_completeness,
                hallucination_count,
                queries: l3_queries,
            });
        }
    }

    // ── Build and print report ──
    let eval_run = EvalRun {
        run_id: run_id.clone(),
        timestamp,
        api_url,
        total_queries: queries.len(),
        l1_results,
        l2_flags,
        l3_results,
        failures: all_failures,
    };

    report::print_run_summary(&eval_run);

    std::fs::create_dir_all(&output_dir)?;
    let report_path = format!("{}/eval-{}.json", output_dir, run_id);
    report::save_json_report(&eval_run, &report_path)?;

    Ok(())
}

async fn print_formulas() -> anyhow::Result<()> {
    let api_url = std::env::var("ZING_EVAL_API_URL")
        .unwrap_or_else(|_| "http://localhost:3001".to_string());
    let client = reqwest::Client::new();

    let search_url = format!("{}/search/formula", api_url);
    let search_resp = client.get(&search_url).send().await?;
    let search_body = search_resp.text().await?;
    println!("─── /search/formula ───");
    println!("{}", search_body);
    println!();

    let chunk_url = format!("{}/chunk/formula", api_url);
    let chunk_resp = client.get(&chunk_url).send().await?;
    let chunk_body = chunk_resp.text().await?;
    println!("─── /chunk/formula ───");
    println!("{}", chunk_body);

    Ok(())
}

fn list_queries(query_dir: Option<String>) -> anyhow::Result<()> {
    let default_dir = std::env::var("ZING_EVAL_QUERY_DIR")
        .unwrap_or_else(|_| "./eval/queries".to_string());
    let path = PathBuf::from(query_dir.unwrap_or(default_dir));
    let queries = golden::load_queries(&path)?;

    for q in &queries {
        let qtype = match q.question_type {
            QuestionType::Factual => "factual".cyan(),
            QuestionType::Procedural => "procedural".yellow(),
            QuestionType::Conceptual => "conceptual".magenta(),
        };
        println!(
            "{}  [{qtype}] {} ({} expected, {} not_expected, L3={})",
            q.id.bold(),
            q.query,
            q.expected_chunks.len(),
            q.not_expected.len(),
            if q.ground_truth_answer.is_some() {
                "✓".green()
            } else {
                "✗".dimmed()
            },
            qtype = qtype,
        );

        if let Some(ref note) = q.eval_note {
            println!("     {}", note.dimmed());
        }
    }

    println!(
        "\n{} queries total ({} L3-ready)",
        queries.len(),
        queries
            .iter()
            .filter(|q| q.ground_truth_answer.is_some())
            .count()
    );
    Ok(())
}
