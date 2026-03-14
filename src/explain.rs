use colored::*;

use crate::dependency_graph::DependencyGraph;
use crate::index::RepoIndex;

/// Print explanation of a specific file.
pub fn explain_file(
    file_path: &str,
    index: &RepoIndex,
    graph: &DependencyGraph,
) {
    // Find the file in the index
    let entry_idx = match index.path_index.get(file_path) {
        Some(&idx) => idx,
        None => {
            // Try partial match
            let matches: Vec<(usize, &str)> = index
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.relative_path.contains(file_path))
                .map(|(i, e)| (i, e.relative_path.as_str()))
                .collect();

            if matches.is_empty() {
                eprintln!("{} File not found: {}", "ERROR:".red().bold(), file_path);
                return;
            }
            if matches.len() > 1 {
                eprintln!("{} Ambiguous path '{}', matches:", "WARNING:".yellow().bold(), file_path);
                for (_, path) in &matches {
                    eprintln!("  - {}", path);
                }
                eprintln!("Using first match.");
            }
            matches[0].0
        }
    };

    let entry = &index.entries[entry_idx];

    println!();
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!("  {} {}", "FILE:".bright_white().bold(), entry.relative_path.bright_green().bold());
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!();

    println!("  {} {}", "LANGUAGE:".bright_white().bold(), entry.language.name().bright_yellow());
    println!("  {} {} bytes", "SIZE:".bright_white().bold(), entry.size.to_string().bright_yellow());
    println!(
        "  {} {}",
        "TOKENS:".bright_white().bold(),
        crate::token::format_token_count(entry.token_count).bright_yellow()
    );
    println!(
        "  {} {:.1}",
        "IMPORTANCE:".bright_white().bold(),
        entry.importance.to_string().bright_yellow()
    );
    println!();

    // Imports
    if !entry.imports.is_empty() {
        println!("  {}", "IMPORTS:".bright_white().bold());
        for imp in &entry.imports {
            println!("    {} {}", "→".bright_blue(), imp.bright_white());
        }
        println!();
    }

    // Exported symbols
    if !entry.symbols.is_empty() {
        println!("  {}", "EXPORTED SYMBOLS:".bright_white().bold());
        for sym in &entry.symbols {
            println!("    {} {}", "●".bright_green(), sym.bright_white());
        }
        println!();
    }

    // Dependents (files that import this file)
    let dependents = graph.dependents(entry_idx);
    if !dependents.is_empty() {
        println!("  {}", "USED BY:".bright_white().bold());
        for &dep_idx in dependents {
            let dep_entry = &index.entries[dep_idx];
            println!("    {} {}", "←".bright_magenta(), dep_entry.relative_path.bright_white());
        }
        println!();
    }

    // Dependencies
    let dependencies = graph.dependencies(entry_idx);
    if !dependencies.is_empty() {
        println!("  {}", "DEPENDS ON:".bright_white().bold());
        for &dep_idx in dependencies {
            let dep_entry = &index.entries[dep_idx];
            println!("    {} {}", "→".bright_blue(), dep_entry.relative_path.bright_white());
        }
        println!();
    }
}
