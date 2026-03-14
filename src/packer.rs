use std::collections::{BTreeMap, HashMap};

use chrono::Local;

use crate::index::FileEntry;
use crate::scanner::ScannedFile;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Default,
    Ai,
}

/// Detect duplicate files by blake3 hash. Returns a map of hash → list of paths.
fn detect_duplicates(entries: &[&FileEntry], content_map: &HashMap<&str, &str>) -> HashMap<String, Vec<String>> {
    let mut hash_to_paths: HashMap<String, Vec<String>> = HashMap::new();

    for entry in entries {
        if let Some(content) = content_map.get(entry.relative_path.as_str()) {
            let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
            hash_to_paths
                .entry(hash)
                .or_default()
                .push(entry.relative_path.clone());
        }
    }

    // Only keep entries with more than one file (actual duplicates)
    hash_to_paths.retain(|_, paths| paths.len() > 1);
    hash_to_paths
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
    let content_map: HashMap<&str, &str> = files
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
                if total + entry.token_count <= budget + (budget / 10) {
                    selected.push(*entry);
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

    // Detect duplicates
    let duplicates = detect_duplicates(&output_entries, &content_map);

    // Build reverse map: path → "duplicate of <canonical>"
    let mut duplicate_of: HashMap<&str, &str> = HashMap::new();
    for paths in duplicates.values() {
        // First path is canonical, rest are duplicates
        let canonical = &paths[0];
        for dup_path in &paths[1..] {
            duplicate_of.insert(dup_path.as_str(), canonical.as_str());
        }
    }

    match format {
        OutputFormat::Default => pack_default(repo_name, &output_entries, &content_map, &duplicate_of),
        OutputFormat::Ai => pack_ai(repo_name, &output_entries, &content_map, &duplicate_of),
    }
}

/// Build a compact file index: one line per file with all metadata.
fn build_file_index(entries: &[&FileEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        // Format: path | language | tokens | imports
        out.push_str(&entry.relative_path);
        out.push_str(" | ");
        out.push_str(&format!("{}", entry.language));
        out.push_str(" | ");
        out.push_str(&entry.token_count.to_string());
        if !entry.imports.is_empty() {
            out.push_str(" | ");
            out.push_str(&entry.imports.join(", "));
        }
        out.push('\n');
    }
    out
}

/// Default LLM-native text format — token-efficient.
fn pack_default(
    repo_name: &str,
    entries: &[&FileEntry],
    content_map: &HashMap<&str, &str>,
    duplicate_of: &HashMap<&str, &str>,
) -> String {
    let total_tokens: usize = entries.iter().map(|e| e.token_count).sum();
    let file_count = entries.len();
    let dup_count = duplicate_of.len();

    let mut output = String::with_capacity(total_tokens * 4);

    // Compact header
    output.push_str(&format!(
        "# {} | v{} | {} | files:{} | tokens:{}\n\n",
        repo_name,
        env!("CARGO_PKG_VERSION"),
        Local::now().format("%Y-%m-%d"),
        file_count,
        total_tokens
    ));

    // Compact file index
    output.push_str("## FILE INDEX\n");
    output.push_str(&build_file_index(entries));

    // Note duplicates in the index section
    if dup_count > 0 {
        output.push_str(&format!("\n## DUPLICATES ({})\n", dup_count));
        for (dup_path, canonical) in duplicate_of {
            output.push_str(&format!("{} = {}\n", dup_path, canonical));
        }
    }

    output.push('\n');

    // File contents — minimal delimiters
    for entry in entries {
        let content = content_map.get(entry.relative_path.as_str()).unwrap_or(&"");

        // If this file is a duplicate, just note it — don't repeat the content
        if let Some(canonical) = duplicate_of.get(entry.relative_path.as_str()) {
            output.push_str(&format!("--- {} [duplicate of {}] ---\n\n", entry.relative_path, canonical));
            continue;
        }

        output.push_str(&format!("--- {} ---\n", entry.relative_path));
        output.push_str(content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
        output.push('\n');
    }

    output
}

/// Compact AI format for maximum token reduction.
fn pack_ai(
    repo_name: &str,
    entries: &[&FileEntry],
    content_map: &HashMap<&str, &str>,
    duplicate_of: &HashMap<&str, &str>,
) -> String {
    let mut output = String::new();

    output.push_str(&format!("@repo {} v{} {}\n", repo_name, env!("CARGO_PKG_VERSION"), Local::now().format("%Y-%m-%d")));

    // Compact index
    for entry in entries {
        if let Some(canonical) = duplicate_of.get(entry.relative_path.as_str()) {
            output.push_str(&format!("@dup {} = {}\n", entry.relative_path, canonical));
            continue;
        }

        let content = content_map.get(entry.relative_path.as_str()).unwrap_or(&"");
        output.push_str(&format!("@f {}\n", entry.relative_path));
        output.push_str(content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("@/f\n");
    }

    output
}

/// Build a directory tree string from file paths.
pub fn build_directory_tree(paths: &[&str]) -> String {
    let mut tree: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 1 {
            tree.entry(String::new()).or_default().push(parts[0].to_string());
        } else {
            for i in 0..parts.len() - 1 {
                let dir = parts[..=i].join("/");
                let _ = tree.entry(dir).or_default();
            }
            let parent = parts[..parts.len() - 1].join("/");
            tree.entry(parent).or_default().push(parts.last().unwrap().to_string());
        }
    }

    let mut result = String::new();
    let mut rendered: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        for i in 0..parts.len() - 1 {
            let dir_path = parts[..=i].join("/");
            if rendered.insert(dir_path.clone()) {
                let indent = "  ".repeat(i);
                result.push_str(&format!("{}{}/\n", indent, parts[i]));
            }
        }
        let indent = "  ".repeat(parts.len() - 1);
        result.push_str(&format!("{}{}\n", indent, parts.last().unwrap()));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Language;

    #[test]
    fn test_directory_tree() {
        let paths = vec!["src/main.rs", "src/lib.rs", "src/utils/helper.rs", "Cargo.toml"];
        let tree = build_directory_tree(&paths);
        assert!(tree.contains("src/"));
        assert!(tree.contains("main.rs"));
        assert!(tree.contains("Cargo.toml"));
    }

    #[test]
    fn test_file_index_formatting() {
        let entry1 = FileEntry {
            relative_path: "src/main.rs".to_string(),
            language: Language::Rust,
            size: 100,
            token_count: 50,
            imports: vec!["scanner".to_string(), "packer".to_string()],
            symbols: vec!["main".to_string()],
            importance: 30.0,
        };
        let entry2 = FileEntry {
            relative_path: "src/utils.rs".to_string(),
            language: Language::Rust,
            size: 50,
            token_count: 25,
            imports: vec![],
            symbols: vec!["helper".to_string()],
            importance: 5.0,
        };
        let entries: Vec<&FileEntry> = vec![&entry1, &entry2];
        let index = build_file_index(&entries);

        assert!(index.contains("src/main.rs | rust | 50 | scanner, packer"));
        assert!(index.contains("src/utils.rs | rust | 25"));
        // utils has no imports, so no trailing pipe
        assert!(!index.contains("src/utils.rs | rust | 25 |"));
    }

    #[test]
    fn test_duplicate_detection() {
        let entry1 = FileEntry {
            relative_path: "src/a.rs".to_string(),
            language: Language::Rust,
            size: 10,
            token_count: 5,
            imports: vec![],
            symbols: vec![],
            importance: 0.0,
        };
        let entry2 = FileEntry {
            relative_path: "src/b.rs".to_string(),
            language: Language::Rust,
            size: 10,
            token_count: 5,
            imports: vec![],
            symbols: vec![],
            importance: 0.0,
        };
        let entry3 = FileEntry {
            relative_path: "src/c.rs".to_string(),
            language: Language::Rust,
            size: 10,
            token_count: 5,
            imports: vec![],
            symbols: vec![],
            importance: 0.0,
        };

        let entries: Vec<&FileEntry> = vec![&entry1, &entry2, &entry3];
        let mut content_map: HashMap<&str, &str> = HashMap::new();
        content_map.insert("src/a.rs", "fn hello() {}");
        content_map.insert("src/b.rs", "fn hello() {}"); // same content as a.rs
        content_map.insert("src/c.rs", "fn world() {}"); // different

        let dups = detect_duplicates(&entries, &content_map);
        assert_eq!(dups.len(), 1); // one set of duplicates
        let dup_paths = dups.values().next().unwrap();
        assert_eq!(dup_paths.len(), 2); // a.rs and b.rs
    }

    #[test]
    fn test_pack_default_deduplicates_content() {
        let entry1 = FileEntry {
            relative_path: "src/a.rs".to_string(),
            language: Language::Rust,
            size: 13,
            token_count: 5,
            imports: vec![],
            symbols: vec![],
            importance: 0.0,
        };
        let entry2 = FileEntry {
            relative_path: "src/b.rs".to_string(),
            language: Language::Rust,
            size: 13,
            token_count: 5,
            imports: vec![],
            symbols: vec![],
            importance: 0.0,
        };

        let entries: Vec<&FileEntry> = vec![&entry1, &entry2];
        let files = vec![
            ScannedFile {
                path: "src/a.rs".into(),
                relative_path: "src/a.rs".to_string(),
                size: 13,
                content: "fn hello() {}".to_string(),
            },
            ScannedFile {
                path: "src/b.rs".into(),
                relative_path: "src/b.rs".to_string(),
                size: 13,
                content: "fn hello() {}".to_string(),
            },
        ];

        let output = pack("test", &entries, &files, OutputFormat::Default, None);

        // Content should only appear once
        let occurrences = output.matches("fn hello() {}").count();
        assert_eq!(occurrences, 1, "Duplicate content should only appear once");

        // The duplicate marker should exist
        assert!(output.contains("duplicate of"), "Should note the duplicate");
    }
}
