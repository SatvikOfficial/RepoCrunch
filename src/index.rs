use std::collections::HashMap;
use std::path::Path;

use rayon::prelude::*;

use crate::language::{self, Language, ParsedFile};
use crate::scanner::ScannedFile;
use crate::token;

/// A single file entry in the repository index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub relative_path: String,
    pub language: Language,
    pub size: u64,
    pub token_count: usize,
    pub imports: Vec<String>,
    pub symbols: Vec<String>,
    pub importance: f64,
}

/// The complete repository index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepoIndex {
    pub entries: Vec<FileEntry>,
    /// Map from relative path → index in entries vec
    #[serde(skip)]
    pub path_index: HashMap<String, usize>,
}

impl RepoIndex {
    /// Build the index from scanned files. Parses each file with Tree-sitter in parallel.
    pub fn build(files: &[ScannedFile]) -> Self {
        let entries: Vec<FileEntry> = files
            .par_iter()
            .map(|f| {
                let path = Path::new(&f.relative_path);
                let first_line = f.content.lines().next();
                let lang = language::detect_language(path, first_line);

                // Parse with Tree-sitter
                let parsed: ParsedFile = language::parse_file(lang, &f.content);

                // Count tokens in parallel-friendly way
                let token_count = token::count_tokens(&f.content);

                FileEntry {
                    relative_path: f.relative_path.clone(),
                    language: lang,
                    size: f.size,
                    token_count,
                    imports: parsed.imports,
                    symbols: parsed.symbols,
                    importance: 0.0, // computed later
                }
            })
            .collect();

        let path_index: HashMap<String, usize> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.relative_path.clone(), i))
            .collect();

        let mut index = RepoIndex {
            entries,
            path_index,
        };

        // Compute importance scores
        index.compute_importance();
        index
    }

    /// Rebuild the path_index from entries (e.g., after deserialization).
    pub fn rebuild_path_index(&mut self) {
        self.path_index = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.relative_path.clone(), i))
            .collect();
    }

    /// Compute importance scores for each file.
    fn compute_importance(&mut self) {
        let max_imports = self
            .entries
            .iter()
            .map(|e| e.imports.len())
            .max()
            .unwrap_or(1)
            .max(1) as f64;

        let max_symbols = self
            .entries
            .iter()
            .map(|e| e.symbols.len())
            .max()
            .unwrap_or(1)
            .max(1) as f64;

        // Count how many files import each file (dependent count)
        let mut dependent_count: HashMap<String, usize> = HashMap::new();
        for entry in &self.entries {
            for imp in &entry.imports {
                *dependent_count.entry(imp.clone()).or_insert(0) += 1;
            }
        }

        let max_deps = *dependent_count.values().max().unwrap_or(&1).max(&1) as f64;

        for entry in &mut self.entries {
            let mut score = 0.0;

            // Entrypoint files get a boost
            let name = Path::new(&entry.relative_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if matches!(name, "main" | "index" | "app" | "lib" | "mod" | "server" | "cli") {
                score += 30.0;
            }

            // Files with many exports/symbols are important
            score += (entry.symbols.len() as f64 / max_symbols) * 20.0;

            // Files with many imports (complex/orchestrating)
            score += (entry.imports.len() as f64 / max_imports) * 10.0;

            // Files that many other files depend on
            let deps = dependent_count
                .get(&entry.relative_path)
                .copied()
                .unwrap_or(0);
            score += (deps as f64 / max_deps) * 25.0;

            // Small utility files get a small boost (they are cheap to include)
            if entry.token_count < 200 {
                score += 5.0;
            }

            entry.importance = score;
        }
    }

    /// Get total token count.
    pub fn total_tokens(&self) -> usize {
        self.entries.iter().map(|e| e.token_count).sum()
    }

    /// Get language breakdown.
    pub fn language_breakdown(&self) -> HashMap<Language, usize> {
        let mut map: HashMap<Language, usize> = HashMap::new();
        for entry in &self.entries {
            *map.entry(entry.language).or_insert(0) += 1;
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importance_entrypoint() {
        let files = vec![
            ScannedFile {
                path: "src/main.rs".into(),
                relative_path: "src/main.rs".to_string(),
                size: 100,
                content: "fn main() {}".to_string(),
            },
            ScannedFile {
                path: "src/utils.rs".into(),
                relative_path: "src/utils.rs".to_string(),
                size: 50,
                content: "pub fn helper() {}".to_string(),
            },
        ];

        let index = RepoIndex::build(&files);
        let main_entry = index.entries.iter().find(|e| e.relative_path == "src/main.rs").unwrap();
        let utils_entry = index.entries.iter().find(|e| e.relative_path == "src/utils.rs").unwrap();

        // main.rs should have higher importance
        assert!(main_entry.importance > utils_entry.importance);
    }
}
