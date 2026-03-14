# RepoCrunch 🚀

A high-performance repository packer and query engine written in Rust, designed to prepare codebases for Large Language Models (LLMs).

It recursively scans your repository, builds an intelligent file index utilizing **Tree-sitter** for parsing, and packs relevant source code into a token-efficient, LLM-native format.

## Features

- ⚡ **Extremely Fast** — Zero-copy I/O (`memmap2`), parallel scanning (`rayon`), sub-second cached runs
- 🧠 **Tree-sitter Parsing** — Accurate import/symbol extraction for Rust, TypeScript, JavaScript, Python, Go, Java, C/C++
- 🔎 **Query Mode** — `repocrunch "auth files"` packs only relevant files with heuristic query expansion
- 📊 **Dependency Graph** — Understands cross-file imports and resolves missing context automatically
- 🗂️ **Compact Output** — FILE INDEX header, `--- path ---` delimiters, blake3 duplicate detection
- 💾 **Smart Caching** — `blake3` content hashing for 8× speedups on subsequent runs
- 📋 **Clipboard Support** — `--copy` sends output directly to clipboard
- 🔧 **Utility Commands** — `stats`, `explain`, `flow` for deep repository inspection

## Installation

```bash
# From source
git clone https://github.com/SatvikOfficial/RepoCrunch.git
cd RepoCrunch
cargo install --path .
```

## Usage

Run `repocrunch` anywhere inside a Git repository. It automatically detects the repo root by walking up to find `.git`.

### Pack Modes

```bash
# Full repository pack
repocrunch

# Query-filtered pack (natural language)
repocrunch "files related to authentication"
repocrunch "frontend components"
repocrunch "database models"
```

### Options

| Flag | Description |
|------|-------------|
| `--format ai` | Ultra-compact `@f`/`@/f` output format |
| `--max-tokens N` | Enforce a token budget (importance-based selection) |
| `--max-file-size N` | Override max file size in bytes (default: 1MB) |
| `--copy` | Copy output to clipboard |
| `--json-index` | Export file index as JSON for tool integration |
| `--no-cache` | Disable blake3 caching |
| `-o FILE` | Output file path (default: `repocrunch-output.txt`) |
| `--verbose` | Show timing details |

### Subcommands

```bash
# Repository statistics with language breakdown
repocrunch stats

# Deep file inspection (imports, symbols, dependents)
repocrunch explain src/main.rs

# Trace execution flow through the dependency graph
repocrunch flow login
```

## Output Format

RepoCrunch generates a token-efficient format optimized for LLM consumption:

```
# MyProject | v0.1.0 | 2026-03-14 | files:15 | tokens:37021

## FILE INDEX
src/main.rs | rust | 2488 | scanner, packer, query
src/scanner.rs | rust | 1977 | ignore::WalkBuilder, memmap2::Mmap
src/utils.rs | rust | 200

## DUPLICATES (1)
src/copy.rs = src/original.rs

--- src/main.rs ---
fn main() { ... }

--- src/copy.rs [duplicate of src/original.rs] ---

--- src/original.rs ---
fn original() { ... }
```

**Key design decisions:**
- Compact FILE INDEX at the top replaces per-file metadata headers
- One line per file: `path | language | tokens | imports`
- `--- path ---` delimiters are simple and token-efficient
- Duplicate files detected via blake3 hash — content included only once
- No XML, no `<code>` tags, no verbose formatting

## `.repocrunchignore`

Drop a `.repocrunchignore` file in your repo root to exclude paths from packing. Uses the same syntax as `.gitignore`:

```
docs/
*.log
tests/
*.min.js
```

## Performance

| Metric | Value |
|--------|-------|
| First run (15 files) | ~94ms |
| Cached run | ~6ms |
| Token reduction vs XML format | **56%** |
| Parallel threads | Auto-detected |

## Architecture

```
scanner.rs → language.rs → index.rs → dependency_graph.rs → query.rs → packer.rs
                                          ↑
                                       cache.rs (blake3)
```

Plus: `token.rs`, `stats.rs`, `explain.rs`, `flow.rs`, `main.rs`

## License

MIT
