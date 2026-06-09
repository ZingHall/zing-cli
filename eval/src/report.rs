use crate::types::*;
use colored::Colorize;

pub fn print_run_summary(run: &EvalRun) {
    println!();
    println!("{}", "═══ Zing Eval Report ═══".bold().cyan());
    println!("Run ID:   {}", run.run_id);
    println!("Time:     {}", run.timestamp);
    println!("API:      {}", run.api_url);
    println!("Queries:  {}", run.total_queries);
    println!();

    if let Some(ref l1) = run.l1_results {
        print_l1_summary(l1);
    }
    if let Some(ref l2) = run.l2_flags {
        print_l2_summary(l2);
    }
    if let Some(ref l3) = run.l3_results {
        print_l3_summary(l3);
    }
    if !run.failures.is_empty() {
        print_failure_catalog(&run.failures);
    }
}

fn print_l1_summary(l1: &Level1Results) {
    println!("{}", "─── Level 1: Retrieval Quality ───".bold().yellow());
    let pct = if l1.queries.is_empty() {
        0.0
    } else {
        l1.passed as f64 / l1.queries.len() as f64 * 100.0
    };
    let status_color = if pct >= 90.0 { "green" } else { "red" };
    println!(
        "Passed: {} | Failed: {} | Rate: {:.1}%",
        l1.passed.to_string().color(status_color),
        l1.failed.to_string().red(),
        pct
    );
    println!();

    for q in &l1.queries {
        let icon = if q.status == "PASS" { "✓".green() } else { "✗".red() };
        println!(
            "  {} {} ({})",
            icon,
            q.query_id.bold(),
            q.endpoint.dimmed()
        );
        for check in &q.checks {
            let check_icon = if check.status.contains("PASS") {
                "  ✓".green()
            } else {
                "  ✗".red()
            };
            println!("    {} {} — {}", check_icon, check.check_name, check.detail);
        }
        if let Some(ref details) = q.details.best_text_snippet {
            println!("    {} best excerpt: {}", "▶".dimmed(), details.dimmed());
        }
        if let Some(ref violations) = q.details.not_expected_violations {
            for v in violations {
                println!("    {} {}", "⚠".yellow(), v.yellow());
            }
        }
        println!();
    }
}

fn print_l2_summary(flags: &[Level2Flag]) {
    println!("{}", "─── Level 2: Score Sanity ───".bold().yellow());
    if flags.is_empty() {
        println!("  {} No score sanity issues detected.", "✓".green());
        println!();
        return;
    }

    let mut by_type: std::collections::HashMap<&str, Vec<&Level2Flag>> =
        std::collections::HashMap::new();
    for f in flags {
        by_type.entry(&f.flag_type).or_default().push(f);
    }

    for (flag_type, entries) in &by_type {
        println!("  {} {} ({} queries)", "⚠".yellow(), flag_type, entries.len());
        for e in entries {
            println!("    └─ query={}: {}", e.query_id, e.detail);
        }
    }
    println!();
}

fn print_l3_summary(l3: &Level3Results) {
    println!("{}", "─── Level 3: RAG Answer Quality ───".bold().yellow());
    println!(
        "  Avg factual accuracy: {:.1}/5",
        l3.avg_factual_accuracy
    );
    println!("  Avg completeness:     {:.1}/5", l3.avg_completeness);
    println!(
        "  Hallucinations:       {}",
        if l3.hallucination_count > 0 {
            l3.hallucination_count.to_string().red()
        } else {
            l3.hallucination_count.to_string().green()
        }
    );
    println!();

    for q in &l3.queries {
        let h_icon = if q.hallucination {
            "HALLUCINATION".red().bold()
        } else {
            "OK".green()
        };
        println!(
            "  {} accuracy={:.1}/5 completeness={:.1}/5 {}",
            q.query_id.bold(),
            q.factual_accuracy,
            q.completeness,
            h_icon
        );
        if !q.judge_comment.is_empty() {
            println!("    {}", q.judge_comment.dimmed());
        }
    }
    println!();
}

fn print_failure_catalog(failures: &[FailureEntry]) {
    println!(
        "{}",
        "─── Level 4: Failure Catalog ───".bold().yellow()
    );
    let mut by_category: std::collections::HashMap<&str, Vec<&FailureEntry>> =
        std::collections::HashMap::new();
    for f in failures {
        let cat = match f.category {
            FailureCategory::RetrievalFailure => "retrieval",
            FailureCategory::RankingFailure => "ranking",
            FailureCategory::ContextFailure => "context",
            FailureCategory::Hallucination => "hallucination",
            FailureCategory::Truncation => "truncation",
        };
        by_category.entry(cat).or_default().push(f);
    }

    let categories = [
        "retrieval",
        "ranking",
        "context",
        "hallucination",
        "truncation",
    ];
    for cat in &categories {
        if let Some(entries) = by_category.get(cat) {
            let fix_hint = match *cat {
                "retrieval" => "fix: adjust chunk size, query expansion, BM25 config",
                "ranking" => "fix: tune blend weights, reranker threshold",
                "context" => "fix: prompt formatting, context window management",
                "hallucination" => "fix: stricter system prompt, temperature reduction",
                "truncation" => "fix: auto_expand for structured content",
                _ => "",
            };
            println!(
                "  {} {} x{} — {}",
                "✗".red(),
                cat.bold(),
                entries.len(),
                fix_hint.dimmed()
            );
            for e in entries {
                println!("    └─ {}: {}", e.query_id, e.detail);
            }
        }
    }
    println!();
}

pub fn save_json_report(run: &EvalRun, path: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(run)?;
    std::fs::write(path, json)?;
    println!("Report saved to {}", path.dimmed());
    Ok(())
}
