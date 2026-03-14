use colored::*;

use crate::dependency_graph::DependencyGraph;
use crate::index::RepoIndex;

/// Trace and print the execution flow for a symbol or file.
pub fn trace_flow(
    query: &str,
    index: &RepoIndex,
    graph: &DependencyGraph,
) {
    // Find files that contain the query symbol
    let matches: Vec<usize> = index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            // Match by symbol name
            e.symbols.iter().any(|s| s.to_lowercase().contains(&query.to_lowercase()))
                // or by filename
                || e.relative_path.to_lowercase().contains(&query.to_lowercase())
        })
        .map(|(i, _)| i)
        .collect();

    if matches.is_empty() {
        eprintln!("{} No files matching '{}' found.", "ERROR:".red().bold(), query);
        return;
    }

    println!();
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!("  {} {}", "FLOW TRACE:".bright_white().bold(), query.bright_green().bold());
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!();

    for start_idx in &matches {
        let flow = graph.trace_flow(*start_idx, index, 6);
        for node in &flow {
            let indent = "  ".repeat(node.depth + 1);
            let arrow = if node.depth == 0 {
                "●".bright_green().to_string()
            } else {
                "→".bright_blue().to_string()
            };
            println!("{}{} {}", indent, arrow, node.path.bright_white());
        }
        println!();
    }
}
