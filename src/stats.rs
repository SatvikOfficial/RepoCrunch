use colored::*;

use crate::index::RepoIndex;
use crate::token;

/// Print repo statistics to stdout.
pub fn print_stats(repo_name: &str, index: &RepoIndex) {
    let total_files = index.entries.len();
    let total_tokens = index.total_tokens();
    let lang_breakdown = index.language_breakdown();

    println!();
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!("  {} {}", "REPO:".bright_white().bold(), repo_name.bright_green().bold());
    println!("{}", "══════════════════════════════════════".bright_cyan());
    println!();

    println!("  {} {}", "FILES:".bright_white().bold(), total_files.to_string().bright_yellow());
    println!("  {} {}", "TOKENS (EST):".bright_white().bold(), token::format_token_count(total_tokens).bright_yellow());
    println!();

    println!("  {}", "LANGUAGES:".bright_white().bold());

    // Sort by count descending
    let mut langs: Vec<_> = lang_breakdown.into_iter().collect();
    langs.sort_by(|a, b| b.1.cmp(&a.1));

    for (lang, count) in &langs {
        let max_count = langs.first().map(|(_, c)| *c).unwrap_or(1).max(1) as f64;
        let bar_len = (*count as f64 / max_count * 20.0) as usize;
        let bar = "█".repeat(bar_len.max(1));
        println!(
            "    {:<15} {:>4}  {}",
            lang.name().bright_white(),
            count.to_string().bright_yellow(),
            bar.bright_blue()
        );
    }

    println!();

    // Top 5 largest files by tokens
    let mut by_tokens: Vec<_> = index.entries.iter().collect();
    by_tokens.sort_by(|a, b| b.token_count.cmp(&a.token_count));

    println!("  {}", "LARGEST FILES (by tokens):".bright_white().bold());
    for entry in by_tokens.iter().take(5) {
        println!(
            "    {} {} tokens",
            entry.relative_path.bright_white(),
            token::format_token_count(entry.token_count).bright_yellow()
        );
    }

    println!();
}
