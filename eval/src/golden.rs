use crate::types::GoldenQuery;
use anyhow::Context;
use std::path::Path;

pub fn load_queries(dir: &Path) -> anyhow::Result<Vec<GoldenQuery>> {
    let mut queries = Vec::new();
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read query directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if !matches!(ext, Some("yaml") | Some("yml")) {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Cannot read file: {}", path.display()))?;
        let query: GoldenQuery = serde_yaml::from_str(&content)
            .with_context(|| format!("Invalid YAML in: {}", path.display()))?;
        queries.push(query);
    }

    queries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(queries)
}
