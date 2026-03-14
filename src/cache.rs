use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::index::RepoIndex;

const CACHE_DIR: &str = ".repocrunch";
const INDEX_FILE: &str = "index.json";
const HASHES_FILE: &str = "file_hashes.json";

#[derive(Debug, Serialize, Deserialize)]
struct FileHashes {
    hashes: HashMap<String, String>,
}

/// Get the cache directory path.
fn cache_dir(root: &Path) -> PathBuf {
    root.join(CACHE_DIR)
}

/// Compute a blake3 hash of file content.
pub fn hash_content(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

/// Try to load a cached index. Returns None if cache is stale or missing.
pub fn load_cached_index(root: &Path, current_files: &[(String, String)]) -> Option<RepoIndex> {
    let dir = cache_dir(root);
    if !dir.exists() {
        return None;
    }

    // Load previous hashes
    let hashes_path = dir.join(HASHES_FILE);
    let old_hashes: FileHashes = match fs::read_to_string(&hashes_path) {
        Ok(s) => serde_json::from_str(&s).ok()?,
        Err(_) => return None,
    };

    // Check if all files match
    if current_files.len() != old_hashes.hashes.len() {
        return None;
    }

    for (path, hash) in current_files {
        match old_hashes.hashes.get(path) {
            Some(old_hash) if old_hash == hash => continue,
            _ => return None,
        }
    }

    // Load cached index
    let index_path = dir.join(INDEX_FILE);
    let index_str = fs::read_to_string(&index_path).ok()?;
    let mut index: RepoIndex = serde_json::from_str(&index_str).ok()?;
    index.rebuild_path_index();

    Some(index)
}

/// Save the index and file hashes to cache.
pub fn save_cache(root: &Path, index: &RepoIndex, file_hashes: &[(String, String)]) -> Result<()> {
    let dir = cache_dir(root);
    fs::create_dir_all(&dir)?;

    // Save index
    let index_str = serde_json::to_string(index)?;
    fs::write(dir.join(INDEX_FILE), index_str)?;

    // Save hashes
    let hashes = FileHashes {
        hashes: file_hashes.iter().cloned().collect(),
    };
    let hashes_str = serde_json::to_string(&hashes)?;
    fs::write(dir.join(HASHES_FILE), hashes_str)?;

    Ok(())
}

/// Clear the cache directory.
pub fn clear_cache(root: &Path) -> Result<()> {
    let dir = cache_dir(root);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
