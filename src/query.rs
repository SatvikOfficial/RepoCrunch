use std::collections::HashMap;

use crate::dependency_graph::DependencyGraph;
use crate::index::RepoIndex;

/// Heuristic query expansions for common domain terms.
fn query_expansions() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert("frontend", vec!["component", "page", "ui", "view", "jsx", "tsx", "react", "vue", "angular", "layout", "widget", "render"]);
    m.insert("backend", vec!["api", "route", "controller", "handler", "server", "middleware", "endpoint", "service"]);
    m.insert("database", vec!["model", "migration", "schema", "query", "repo", "entity", "orm", "sql", "db"]);
    m.insert("auth", vec!["login", "token", "jwt", "session", "password", "oauth", "signup", "register", "credential"]);
    m.insert("test", vec!["spec", "test", "mock", "fixture", "assert", "expect", "describe", "it"]);
    m.insert("config", vec!["settings", "env", "configuration", "options", "preferences"]);
    m.insert("style", vec!["css", "scss", "sass", "theme", "color", "font", "layout", "responsive"]);
    m.insert("deploy", vec!["docker", "ci", "cd", "pipeline", "build", "release", "kubernetes", "helm"]);
    m.insert("security", vec!["auth", "encrypt", "decrypt", "hash", "salt", "permission", "role", "access"]);
    m.insert("api", vec!["endpoint", "route", "handler", "controller", "rest", "graphql", "grpc"]);
    m
}

/// Weights for different match types.
const FILENAME_WEIGHT: f64 = 10.0;
const FOLDER_WEIGHT: f64 = 6.0;
const SYMBOL_WEIGHT: f64 = 5.0;
const IMPORT_WEIGHT: f64 = 4.0;
const CONTENT_WEIGHT: f64 = 3.0;

/// A scored file result.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub index: usize,
    pub score: f64,
    pub path: String,
}

/// Execute a query against the repo index using weighted scoring.
pub fn search(
    query: &str,
    index: &RepoIndex,
    graph: &DependencyGraph,
    file_contents: &HashMap<String, String>,
) -> Vec<QueryResult> {
    // Tokenize query and expand heuristics
    let terms = expand_query(query);

    if terms.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<QueryResult> = index
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let score = score_file(entry, &terms, file_contents);
            QueryResult {
                index: i,
                score,
                path: entry.relative_path.clone(),
            }
        })
        .filter(|r| r.score > 0.0)
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Take top results
    let top_count = (results.len().min(50)).max(1);
    let top_results: Vec<QueryResult> = results.into_iter().take(top_count).collect();

    // Dependency expansion: include direct deps/dependents of top 10
    let top_indices: Vec<usize> = top_results.iter().take(10).map(|r| r.index).collect();
    let expanded = graph.expand(&top_indices);

    // Add expanded files that aren't already in results
    let existing: std::collections::HashSet<usize> = top_results.iter().map(|r| r.index).collect();
    let mut final_results = top_results;

    for idx in expanded {
        if !existing.contains(&idx) {
            let entry = &index.entries[idx];
            final_results.push(QueryResult {
                index: idx,
                score: 0.5, // low score for dependency-expanded files
                path: entry.relative_path.clone(),
            });
        }
    }

    // Re-sort
    final_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    final_results
}

/// Expand query terms using heuristic domain mappings.
fn expand_query(query: &str) -> Vec<String> {
    let expansions = query_expansions();
    let mut terms: Vec<String> = Vec::new();

    for word in query.split_whitespace() {
        let lower = word.to_lowercase();

        // Skip common stop words
        if matches!(lower.as_str(), "files" | "related" | "to" | "the" | "a" | "an" | "in" | "of" | "for" | "with" | "and" | "or") {
            continue;
        }

        terms.push(lower.clone());

        // Add expansions
        if let Some(expanded) = expansions.get(lower.as_str()) {
            for &t in expanded {
                terms.push(t.to_string());
            }
        }
    }

    terms
}

/// Score a single file against query terms using weighted matching.
fn score_file(
    entry: &crate::index::FileEntry,
    terms: &[String],
    file_contents: &HashMap<String, String>,
) -> f64 {
    let mut score = 0.0;
    let path_lower = entry.relative_path.to_lowercase();

    // Extract filename and folder components
    let parts: Vec<&str> = path_lower.split('/').collect();
    let filename = parts.last().unwrap_or(&"");
    let folders: Vec<&str> = if parts.len() > 1 {
        parts[..parts.len() - 1].to_vec()
    } else {
        Vec::new()
    };

    let symbols_lower: Vec<String> = entry.symbols.iter().map(|s| s.to_lowercase()).collect();
    let imports_lower: Vec<String> = entry.imports.iter().map(|s| s.to_lowercase()).collect();

    for term in terms {
        // Filename match
        if filename.contains(term.as_str()) {
            score += FILENAME_WEIGHT;
        }

        // Folder match
        for folder in &folders {
            if folder.contains(term.as_str()) {
                score += FOLDER_WEIGHT;
                break;
            }
        }

        // Symbol match
        for sym in &symbols_lower {
            if sym.contains(term.as_str()) {
                score += SYMBOL_WEIGHT;
                break;
            }
        }

        // Import match
        for imp in &imports_lower {
            if imp.contains(term.as_str()) {
                score += IMPORT_WEIGHT;
                break;
            }
        }

        // Content match (lightweight: just check if the term appears)
        if let Some(content) = file_contents.get(&entry.relative_path) {
            if content.to_lowercase().contains(term.as_str()) {
                score += CONTENT_WEIGHT;
            }
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_query_frontend() {
        let terms = expand_query("files related to frontend");
        assert!(terms.contains(&"frontend".to_string()));
        assert!(terms.contains(&"component".to_string()));
        assert!(terms.contains(&"jsx".to_string()));
        // Stop words should be filtered
        assert!(!terms.contains(&"files".to_string()));
        assert!(!terms.contains(&"related".to_string()));
        assert!(!terms.contains(&"to".to_string()));
    }

    #[test]
    fn test_expand_query_auth() {
        let terms = expand_query("auth");
        assert!(terms.contains(&"auth".to_string()));
        assert!(terms.contains(&"login".to_string()));
        assert!(terms.contains(&"jwt".to_string()));
    }
}
