mod cache;
mod dependency_graph;
mod explain;
mod flow;
mod index;
mod language;
mod packer;
mod query;
mod scanner;
mod stats;
mod token;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;


#[derive(Parser)]
#[command(
    name = "repocrunch",
    version,
    about = "High-performance repository packer & query engine for LLMs",
    long_about = "RepoCrunch scans a Git repository, builds an intelligent file index with\n\
                  Tree-sitter parsing, and packs relevant source files into a single\n\
                  LLM-ready output — either for the full repo or filtered by a\n\
                  natural-language query."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Natural-language query to filter files (e.g. "files related to auth")
    query: Option<String>,

    /// Output file path
    #[arg(short, long, default_value = "repocrunch-output.txt")]
    output: String,

    /// Maximum token budget
    #[arg(short = 't', long)]
    max_tokens: Option<usize>,

    /// Output format: default or ai
    #[arg(short, long, default_value = "default")]
    format: String,

    /// Maximum file size in bytes (defaults to 1MB)
    #[arg(long)]
    max_file_size: Option<u64>,

    /// Additional include glob patterns
    #[arg(short, long)]
    include: Vec<String>,

    /// Additional exclude glob patterns
    #[arg(short, long)]
    exclude: Vec<String>,

    /// Print the parsed dependency index as JSON
    #[arg(long)]
    json_index: bool,

    /// Copy output to clipboard
    #[arg(long)]
    copy: bool,

    /// Disable caching
    #[arg(long)]
    no_cache: bool,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show repository statistics
    Stats,
    /// Explain a specific file (imports, symbols, dependents)
    Explain {
        /// File path to explain
        file: String,
    },
    /// Trace execution flow for a symbol or file
    Flow {
        /// Symbol or filename to trace
        symbol: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let start = Instant::now();

    // Determine repo root by walking up to find .git
    let root = scanner::find_repo_root();
    let repo_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();

    // Handle subcommands
    match &cli.command {
        Some(Commands::Stats) => {
            return run_stats(&root, &repo_name, &cli);
        }
        Some(Commands::Explain { file }) => {
            return run_explain(&root, &repo_name, file, &cli);
        }
        Some(Commands::Flow { symbol }) => {
            return run_flow(&root, &repo_name, symbol, &cli);
        }
        None => {}
    }

    // ── Default: Pack mode ───────────────────────────

    let output_format = match cli.format.as_str() {
        "ai" => packer::OutputFormat::Ai,
        _ => packer::OutputFormat::Default,
    };

    // Step 1: Scan
    if !cli.json_index {
        print_step(1, "Scanning repository...");
    }
    let scan_start = Instant::now();
    let files = scanner::scan_repo(&root, cli.max_file_size, &cli.exclude, &cli.include);
    let scan_time = scan_start.elapsed();

    if files.is_empty() {
        eprintln!("{} No source files found.", "WARNING:".yellow().bold());
        return Ok(());
    }

    if cli.verbose {
        println!("  {} files found in {:.0?}", files.len(), scan_time);
    }

    // Step 2: Index (with cache support)
    if !cli.json_index {
        print_step(2, "Building index...");
    }
    let index_start = Instant::now();

    // Compute hashes for cache
    let file_hashes: Vec<(String, String)> = files
        .iter()
        .map(|f| (f.relative_path.clone(), cache::hash_content(f.content.as_bytes())))
        .collect();

    let repo_index = if cli.no_cache {
        index::RepoIndex::build(&files)
    } else {
        match cache::load_cached_index(&root, &file_hashes) {
            Some(cached) => {
                if cli.verbose {
                    println!("  {} Using cached index", "✓".bright_green());
                }
                cached
            }
            None => {
                let idx = index::RepoIndex::build(&files);
                cache::save_cache(&root, &idx, &file_hashes).ok();
                idx
            }
        }
    };

    let index_time = index_start.elapsed();
    if cli.verbose {
        println!("  Indexed {} files in {:.0?}", repo_index.entries.len(), index_time);
    }

    // If JSON index mode, print JSON and exit
    if cli.json_index {
        let json = serde_json::to_string_pretty(&repo_index.entries)?;
        println!("{}", json);
        return Ok(());
    }

    // Step 3: Build dependency graph
    print_step(3, "Building dependency graph...");
    let graph = dependency_graph::DependencyGraph::build(&repo_index);

    // Step 4: Query or full pack
    let selected_entries: Vec<&index::FileEntry> = if let Some(ref query_str) = cli.query {
        print_step(4, &format!("Searching: \"{}\"...", query_str));

        let content_map: HashMap<String, String> = files
            .iter()
            .map(|f| (f.relative_path.clone(), f.content.clone()))
            .collect();

        let results = query::search(query_str, &repo_index, &graph, &content_map);

        if results.is_empty() {
            eprintln!("{} No files matched the query.", "WARNING:".yellow().bold());
            return Ok(());
        }

        if cli.verbose {
            println!("  {} files matched", results.len());
        }

        results
            .iter()
            .filter_map(|r| repo_index.entries.get(r.index))
            .collect()
    } else {
        print_step(4, "Preparing full pack...");
        repo_index.entries.iter().collect()
    };

    // Step 5: Pack
    print_step(5, "Packing output...");
    let output = packer::pack(
        &repo_name,
        &selected_entries,
        &files,
        output_format,
        cli.max_tokens,
    );

    // Write output
    std::fs::write(&cli.output, &output)?;

    // Copy to clipboard if requested
    if cli.copy {
        match copy_to_clipboard(&output) {
            Ok(()) => println!("  {} Copied to clipboard", "✓".bright_green()),
            Err(e) => eprintln!("  {} Failed to copy to clipboard: {}", "✗".red(), e),
        }
    }

    // Summary
    let total_time = start.elapsed();
    let total_tokens: usize = selected_entries.iter().map(|e| e.token_count).sum();

    println!();
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!(
        "  {} {}",
        "OUTPUT:".bright_white().bold(),
        cli.output.bright_green()
    );
    println!(
        "  {} {}",
        "FILES:".bright_white().bold(),
        selected_entries.len().to_string().bright_yellow()
    );
    println!(
        "  {} {}",
        "TOKENS:".bright_white().bold(),
        token::format_token_count(total_tokens).bright_yellow()
    );
    println!(
        "  {} {:.0?}",
        "TIME:".bright_white().bold(),
        total_time
    );
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!();

    Ok(())
}

fn run_stats(root: &PathBuf, repo_name: &str, cli: &Cli) -> Result<()> {
    let files = scanner::scan_repo(root, cli.max_file_size, &cli.exclude, &cli.include);
    let file_hashes: Vec<(String, String)> = files
        .iter()
        .map(|f| (f.relative_path.clone(), cache::hash_content(f.content.as_bytes())))
        .collect();

    let repo_index = if cli.no_cache {
        index::RepoIndex::build(&files)
    } else {
        match cache::load_cached_index(root, &file_hashes) {
            Some(cached) => cached,
            None => {
                let idx = index::RepoIndex::build(&files);
                cache::save_cache(root, &idx, &file_hashes).ok();
                idx
            }
        }
    };

    stats::print_stats(repo_name, &repo_index);
    Ok(())
}

fn run_explain(root: &PathBuf, _repo_name: &str, file: &str, cli: &Cli) -> Result<()> {
    let files = scanner::scan_repo(root, cli.max_file_size, &cli.exclude, &cli.include);
    let file_hashes: Vec<(String, String)> = files
        .iter()
        .map(|f| (f.relative_path.clone(), cache::hash_content(f.content.as_bytes())))
        .collect();

    let repo_index = if cli.no_cache {
        index::RepoIndex::build(&files)
    } else {
        match cache::load_cached_index(root, &file_hashes) {
            Some(cached) => cached,
            None => {
                let idx = index::RepoIndex::build(&files);
                cache::save_cache(root, &idx, &file_hashes).ok();
                idx
            }
        }
    };

    let graph = dependency_graph::DependencyGraph::build(&repo_index);
    explain::explain_file(file, &repo_index, &graph);
    Ok(())
}

fn run_flow(root: &PathBuf, _repo_name: &str, symbol: &str, cli: &Cli) -> Result<()> {
    let files = scanner::scan_repo(root, cli.max_file_size, &cli.exclude, &cli.include);
    let file_hashes: Vec<(String, String)> = files
        .iter()
        .map(|f| (f.relative_path.clone(), cache::hash_content(f.content.as_bytes())))
        .collect();

    let repo_index = if cli.no_cache {
        index::RepoIndex::build(&files)
    } else {
        match cache::load_cached_index(root, &file_hashes) {
            Some(cached) => cached,
            None => {
                let idx = index::RepoIndex::build(&files);
                cache::save_cache(root, &idx, &file_hashes).ok();
                idx
            }
        }
    };

    let graph = dependency_graph::DependencyGraph::build(&repo_index);
    flow::trace_flow(symbol, &repo_index, &graph);
    Ok(())
}

fn print_step(num: u8, msg: &str) {
    println!(
        "  {} {}",
        format!("[{}/5]", num).bright_blue().bold(),
        msg.bright_white()
    );
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}
