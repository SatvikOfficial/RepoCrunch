use std::collections::BTreeMap;

use chrono::Local;

use crate::index::FileEntry;
use crate::scanner::ScannedFile;
use crate::token;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Default,
    Ai,
}

/// Pack the selected files into a formatted string.
pub fn pack(
    repo_name: &str,
    entries: &[&FileEntry],
    files: &[ScannedFile],
    format: OutputFormat,
    max_tokens: Option<usize>,
) -> String {
    // Build a map from relative_path → content
    let content_map: std::collections::HashMap<&str, &str> = files
        .iter()
        .map(|f| (f.relative_path.as_str(), f.content.as_str()))
        .collect();

    // Sort entries by importance (descending) for token budget
    let mut sorted_entries: Vec<&FileEntry> = entries.to_vec();
    sorted_entries.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));

    // Apply token budget if specified
    let selected = if let Some(budget) = max_tokens {
        let mut total = 0usize;
        let mut selected = Vec::new();
        for entry in &sorted_entries {
            if total + entry.token_count > budget {
                // Try to include if there's room
                if total + entry.token_count <= budget + (budget / 10) {
                    selected.push(*entry);
                    total += entry.token_count;
                }
                break;
            }
            selected.push(*entry);
            total += entry.token_count;
        }
        selected
    } else {
        sorted_entries
    };

    // Sort selected back by path for output
    let mut output_entries = selected;
    output_entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    match format {
        OutputFormat::Default => pack_default(repo_name, &output_entries, &content_map),
        OutputFormat::Ai => pack_ai(repo_name, &output_entries, &content_map),
    }
}

/// Build a directory tree string from file paths.
pub fn build_directory_tree(paths: &[&str]) -> String {
    // Build a tree structure
    let mut tree: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 1 {
            tree.entry(String::new()).or_default().push(parts[0].to_string());
        } else {
            // Add all directory levels
            for i in 0..parts.len() - 1 {
                let dir = parts[..=i].join("/");
                let _ = tree.entry(dir).or_default();
            }
            // Add file to its parent directory
            let parent = parts[..parts.len() - 1].join("/");
            tree.entry(parent).or_default().push(parts.last().unwrap().to_string());
        }
    }

    // Render tree
    let mut result = String::new();
    let mut rendered: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        // Render directory components
        for i in 0..parts.len() - 1 {
            let dir_path = parts[..=i].join("/");
            if rendered.insert(dir_path.clone()) {
                let indent = "  ".repeat(i);
                result.push_str(&format!("{}{}/\n", indent, parts[i]));
            }
        }
        // Render file
        let indent = "  ".repeat(parts.len() - 1);
        result.push_str(&format!("{}{}\n", indent, parts.last().unwrap()));
    }

    result
}

/// Default LLM-native text format.
fn pack_default(
    repo_name: &str,
    entries: &[&FileEntry],
    content_map: &std::collections::HashMap<&str, &str>,
) -> String {
    let total_tokens: usize = entries.iter().map(|e| e.token_count).sum();
    let file_count = entries.len();
    let paths: Vec<&str> = entries.iter().map(|e| e.relative_path.as_str()).collect();
    let dir_tree = build_directory_tree(&paths);

    let mut output = String::with_capacity(total_tokens * 5);

    // Header
    output.push_str("======================================\n");
    output.push_str(&format!("REPOCRUNCH_VERSION: {}\n", env!("CARGO_PKG_VERSION")));
    output.push_str(&format!("GENERATED_AT: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    output.push_str(&format!("REPOSITORY: {}\n", repo_name));
    output.push_str(&format!("TOTAL_FILES: {}\n", file_count));
    output.push_str(&format!("EST_TOKENS: {}\n", total_tokens));
    output.push_str("======================================\n\n");

    // Directory tree
    output.push_str("DIRECTORY TREE\n");
    output.push_str("------------------------------\n");
    output.push_str(&dir_tree);
    output.push('\n');

    // Files
    for entry in entries {
        let content = content_map.get(entry.relative_path.as_str()).unwrap_or(&"");

        output.push_str("====================\n");
        output.push_str(&format!("FILE: {}\n", entry.relative_path));
        output.push_str(&format!("LANGUAGE: {}\n", entry.language));
        output.push_str(&format!("TOKENS: {}\n", entry.token_count));
        
        if !entry.imports.is_empty() {
            let imports_str = entry.imports.join(", ");
            output.push_str(&format!("IMPORTS: {}\n", imports_str));
        }
        
        output.push_str("====================\n\n");
        
        output.push_str("<code>\n");
        output.push_str(content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("</code>\n\n");
    }

    output
}

/// Compact AI format for reduced token usage.
fn pack_ai(
    repo_name: &str,
    entries: &[&FileEntry],
    content_map: &std::collections::HashMap<&str, &str>,
) -> String {
    let mut output = String::new();

    output.push_str(&format!("@repo name={}\n", repo_name));
    output.push_str(&format!("@version {}\n", env!("CARGO_PKG_VERSION")));
    output.push_str(&format!("@generated {}\n\n", Local::now().format("%Y-%m-%d %H:%M:%S")));

    for entry in entries {
        let content = content_map.get(entry.relative_path.as_str()).unwrap_or(&"");

        output.push_str(&format!("@file {}\n", entry.relative_path));
        output.push_str(&format!("@lang {}\n", entry.language));

        if !entry.imports.is_empty() {
            let imports: Vec<&str> = entry.imports.iter().map(|s| s.as_str()).collect();
            output.push_str(&format!("@imports {}\n", imports.join(" ")));
        }

        output.push_str("<code>\n");
        output.push_str(content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("</code>\n");
        output.push_str("@end\n\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_tree() {
        let paths = vec!["src/main.rs", "src/lib.rs", "src/utils/helper.rs", "Cargo.toml"];
        let tree = build_directory_tree(&paths);
        assert!(tree.contains("src/"));
        assert!(tree.contains("main.rs"));
        assert!(tree.contains("Cargo.toml"));
    }
}
