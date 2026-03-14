use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ignore::WalkBuilder;
use memmap2::Mmap;
use rayon::prelude::*;

/// A file discovered during scanning.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub size: u64,
    pub content: String,
}

/// Default directory names to ignore.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    "vendor",
    "__pycache__",
    ".next",
    ".cache",
    ".repocrunch",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "pkg",
    "bin",
    "obj",
];

/// Maximum file size to include by default (1 MB).
const DEFAULT_MAX_FILE_SIZE: u64 = 1_048_576;

/// Maximum absolute number of files to process before aborting (prevents infinite hangs)
const MAX_SCANNED_FILES: usize = 50_000;

/// Find the repository root by walking up from the current directory looking for `.git`.
pub fn find_repo_root() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = current_dir.clone();

    #[allow(deprecated)]
    let home = std::env::home_dir();

    loop {
        // Stop walking up if we hit the user's home directory to avoid packing entire filesystems
        if let Some(ref h) = home {
            if dir == *h {
                break;
            }
        }
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }

    // No .git found, return current directory (works for non-git directories)
    current_dir
}

/// Scan a repository root and return all relevant source files.
pub fn scan_repo(root: &Path, max_size: Option<u64>, extra_excludes: &[String], extra_includes: &[String]) -> Vec<ScannedFile> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)          // skip hidden files by default
        .git_ignore(true)      // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .add_custom_ignore_filename(".repocrunchignore") // Support custom ignore file
        .threads(num_cpus());

    let max_file_size = max_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);

    // Add overrides for our default ignores PLUS user includes/excludes
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for dir in IGNORED_DIRS {
        let pattern = format!("!{}/**", dir);
        overrides.add(&pattern).ok();
    }
    for exc in extra_excludes {
        let pattern = format!("!{}", exc);
        overrides.add(&pattern).ok();
    }
    for inc in extra_includes {
        overrides.add(inc).ok();
    }
    if let Ok(ov) = overrides.build() {
        builder.overrides(ov);
    }

    let files: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let file_count = std::sync::atomic::AtomicUsize::new(0);

    builder.build_parallel().run(|| {
        let files = &files;
        let file_count = &file_count;
        Box::new(move |entry| {
            // Safety circuit breaker
            if file_count.load(Ordering::Relaxed) >= MAX_SCANNED_FILES {
                return ignore::WalkState::Quit;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            // STRICT Check: Skip directories, symlinks, sockets, pipes. Only pure files allowed.
            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path().to_path_buf();

            // Skip files over size limit
            if let Ok(meta) = std::fs::metadata(&path) {
                if meta.len() > max_file_size {
                    return ignore::WalkState::Continue;
                }
            }

            files.lock().unwrap().push(path);
            file_count.fetch_add(1, Ordering::Relaxed);
            
            ignore::WalkState::Continue
        })
    });

    let paths = files.into_inner().unwrap();

    // Process files in parallel: detect binary, read content
    let root_path = root.to_path_buf();
    let mut results: Vec<ScannedFile> = paths
        .par_iter()
        .filter_map(|path| {
            // Try memory-mapped read for performance
            let content = read_file_mmap(path)?;

            // Binary detection using `infer`
            if is_binary(path, content.as_bytes()) {
                return None;
            }

            let relative = path
                .strip_prefix(&root_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let size = content.len() as u64;

            Some(ScannedFile {
                path: path.clone(),
                relative_path: relative,
                size,
                content,
            })
        })
        .collect();

    results.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    results
}

/// Read file content using memory-mapped I/O when possible, falling back to regular read.
fn read_file_mmap(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;

    if metadata.len() == 0 {
        return Some(String::new());
    }

    // Use mmap for files > 4KB, regular read for smaller ones
    if metadata.len() > 4096 {
        let mmap = unsafe { Mmap::map(&file).ok()? };
        String::from_utf8(mmap.to_vec()).ok()
    } else {
        std::fs::read_to_string(path).ok()
    }
}

/// Return true if the file appears to be a binary file.
fn is_binary(path: &Path, content: &[u8]) -> bool {
    // Check via `infer` crate (magic bytes)
    if let Some(kind) = infer::get(content) {
        let mime = kind.mime_type();
        // Allow text-based formats
        if mime.starts_with("text/") {
            return false;
        }
        // Allow SVG
        if mime == "image/svg+xml" {
            return false;
        }
        // Everything else infer detects is binary (images, videos, archives, etc.)
        return true;
    }

    // Fallback: check for null bytes in first 8KB
    let check_len = content.len().min(8192);
    if content[..check_len].contains(&0) {
        return true;
    }

    // Fallback: skip known binary extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg"
                | "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac"
                | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
                | "exe" | "dll" | "so" | "dylib"
                | "pdf" | "doc" | "docx" | "xls" | "xlsx"
                | "woff" | "woff2" | "ttf" | "otf" | "eot"
                | "sqlite" | "db"
                | "pyc" | "pyo"
                | "class"
                | "o" | "a" | "lib"
        )
    } else {
        false
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_text() {
        let content = b"fn main() { println!(\"hello\"); }";
        let path = Path::new("test.rs");
        assert!(!is_binary(path, content));
    }

    #[test]
    fn test_is_binary_null_bytes() {
        let mut content = vec![0u8; 100];
        content[50] = 0;
        let path = Path::new("test.bin");
        assert!(is_binary(path, &content));
    }

    #[test]
    fn test_is_binary_extension() {
        let content = b"not really text";
        assert!(is_binary(Path::new("image.png"), content));
        assert!(!is_binary(Path::new("code.rs"), content));
    }
}
