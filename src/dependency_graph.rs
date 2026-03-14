use std::collections::{HashMap, HashSet};

use crate::index::RepoIndex;

/// Directed dependency graph: edges[i] = list of indices that file i depends on.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// file index → set of file indices it imports
    pub edges: Vec<Vec<usize>>,
    /// file index → set of file indices that import it (reverse edges)
    pub reverse_edges: Vec<Vec<usize>>,
}

impl DependencyGraph {
    /// Build a dependency graph from the repo index by resolving imports to file paths.
    pub fn build(index: &RepoIndex) -> Self {
        let n = index.entries.len();
        let mut edges = vec![Vec::new(); n];
        let mut reverse_edges = vec![Vec::new(); n];

        for (i, entry) in index.entries.iter().enumerate() {
            for import in &entry.imports {
                // Try to resolve the import to a file in the index
                if let Some(&target_idx) = resolve_import(import, &entry.relative_path, &index.path_index) {
                    if target_idx != i {
                        edges[i].push(target_idx);
                        reverse_edges[target_idx].push(i);
                    }
                }
            }
        }

        // Deduplicate
        for e in &mut edges {
            e.sort_unstable();
            e.dedup();
        }
        for e in &mut reverse_edges {
            e.sort_unstable();
            e.dedup();
        }

        DependencyGraph { edges, reverse_edges }
    }

    /// Get all direct dependencies of a file.
    pub fn dependencies(&self, idx: usize) -> &[usize] {
        &self.edges[idx]
    }

    /// Get all files that depend on this file.
    pub fn dependents(&self, idx: usize) -> &[usize] {
        &self.reverse_edges[idx]
    }

    /// Expand a set of file indices to include their direct dependencies and dependents.
    pub fn expand(&self, indices: &[usize]) -> Vec<usize> {
        let mut result: HashSet<usize> = indices.iter().copied().collect();
        for &idx in indices {
            for &dep in &self.edges[idx] {
                result.insert(dep);
            }
            for &dep in &self.reverse_edges[idx] {
                result.insert(dep);
            }
        }
        let mut v: Vec<usize> = result.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// Trace the call flow from a starting file through the dependency graph (BFS).
    /// Returns a tree-like structure of paths.
    pub fn trace_flow(&self, start_idx: usize, index: &RepoIndex, max_depth: usize) -> Vec<FlowNode> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        self.trace_recursive(start_idx, index, 0, max_depth, &mut visited, &mut result);
        result
    }

    fn trace_recursive(
        &self,
        idx: usize,
        index: &RepoIndex,
        depth: usize,
        max_depth: usize,
        visited: &mut HashSet<usize>,
        result: &mut Vec<FlowNode>,
    ) {
        if depth > max_depth || visited.contains(&idx) {
            return;
        }
        visited.insert(idx);

        let entry = &index.entries[idx];
        result.push(FlowNode {
            path: entry.relative_path.clone(),
            depth,
        });

        for &dep_idx in &self.edges[idx] {
            self.trace_recursive(dep_idx, index, depth + 1, max_depth, visited, result);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlowNode {
    pub path: String,
    pub depth: usize,
}

/// Try to resolve an import string to a file index in the repo.
fn resolve_import<'a>(
    import: &str,
    _current_file: &str,
    path_index: &'a HashMap<String, usize>,
) -> Option<&'a usize> {
    // Try direct match
    if let Some(idx) = path_index.get(import) {
        return Some(idx);
    }

    // Try common resolutions:
    let normalized = import
        .replace("::", "/") // Rust: std::collections → std/collections
        .replace(".", "/") // Python/Java: os.path → os/path
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_start_matches("crate/")  // Rust: crate::foo → foo
        .trim_start_matches("self/")   // Rust: self::foo → foo
        .trim_start_matches("super/")  // Rust: super::foo → foo
        .to_string();

    // Try with common extensions
    let extensions = &["", ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".cpp", ".c", ".h"];
    for ext in extensions {
        let candidate = format!("{}{}", normalized, ext);
        if let Some(idx) = path_index.get(&candidate) {
            return Some(idx);
        }
        // Try with src/ prefix
        let candidate = format!("src/{}{}", normalized, ext);
        if let Some(idx) = path_index.get(&candidate) {
            return Some(idx);
        }
    }

    // Try matching by filename (last component)
    let filename = import.rsplit('/').next()
        .or_else(|| import.rsplit("::").next())
        .or_else(|| import.rsplit('.').next())
        .unwrap_or(import);

    for ext in extensions {
        let target = format!("{}{}", filename, ext);
        for (path, idx) in path_index {
            if path.ends_with(&target) {
                return Some(idx);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_import_direct() {
        let mut path_index = HashMap::new();
        path_index.insert("src/utils.rs".to_string(), 0);
        path_index.insert("src/main.rs".to_string(), 1);

        let result = resolve_import("src/utils.rs", "src/main.rs", &path_index);
        assert_eq!(result, Some(&0));
    }

    #[test]
    fn test_resolve_import_rust_module() {
        let mut path_index = HashMap::new();
        path_index.insert("src/scanner.rs".to_string(), 0);

        let result = resolve_import("crate::scanner", "src/main.rs", &path_index);
        // Should resolve via filename matching
        assert!(result.is_some());
    }
}
