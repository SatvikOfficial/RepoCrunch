# RepoCrunch 🚀

A high-performance repository packer and query engine written in Rust, designed to prepare codebases for Large Language Models.

Pack your entire repo into a single, token-efficient file that LLMs can actually use — with intelligent file selection, dependency awareness, and query-based filtering.

## Why RepoCrunch?

| | Repomix | RepoCrunch |
|---|---|---|
| **Format** | XML tags (`<file>`, `<code>`) | Flat text (`--- path ---`) |
| **Overhead per file** | ~8 XML wrapper tokens | ~3 delimiter tokens |
| **File metadata** | Repeated per-file headers | Compact FILE INDEX at top |
| **Duplicate detection** | ❌ | ✅ blake3 hash dedup |
| **Query mode** | ❌ | ✅ `repocrunch "auth files"` |
| **Dependency graph** | ❌ | ✅ Tree-sitter imports |
| **Token budget** | ❌ | ✅ `--max-tokens 100000` |
| **Speed (cached)** | — | **6ms** |

### Real Benchmark — Same Repository

| Tool | Tokens | Format Overhead |
|------|--------|-----------------|
| Repomix (XML format) | ~84,000 | XML wrappers + verbose headers |
| **RepoCrunch** (compact) | **~37,000** | Flat index + minimal delimiters |
| **Savings** | **56% fewer tokens** | |

> Tested on the RepoCrunch repository itself (15 source files). Less tokens = more room for your actual prompt.

## Installation

### Quick Install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/SatvikOfficial/RepoCrunch/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/SatvikOfficial/RepoCrunch/main/install.ps1 | iex
```

### Cargo (any platform with Rust)

```bash
cargo install --git https://github.com/SatvikOfficial/RepoCrunch
```

### From Source

```bash
git clone https://github.com/SatvikOfficial/RepoCrunch.git
cd RepoCrunch
cargo install --path .
```

## Usage

Run `repocrunch` anywhere inside a Git repository. It automatically detects the repo root.

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
| `--max-tokens N` | Enforce a token budget (importance-based) |
| `--max-file-size N` | Max file size in bytes (default: 1MB) |
| `--copy` | Copy output to clipboard |
| `--json-index` | Export file index as JSON |
| `--no-cache` | Disable blake3 caching |
| `-o FILE` | Output file path |
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

```
# MyProject | v0.1.0 | 2026-03-14 | files:15 | tokens:37021

## FILE INDEX
src/main.rs | rust | 2488 | scanner, packer, query
src/scanner.rs | rust | 1977 | ignore::WalkBuilder
src/utils.rs | rust | 200

## DUPLICATES (1)
src/copy.rs = src/original.rs

--- src/main.rs ---
fn main() { ... }

--- src/copy.rs [duplicate of src/original.rs] ---

--- src/original.rs ---
fn original() { ... }
```

**Design decisions:**
- One-line-per-file index replaces per-file metadata blocks
- `--- path ---` delimiters are simple, clear, and token-efficient
- Duplicates detected via blake3 — content included only once
- No XML, no `<code>` tags, no verbose formatting

## `.repocrunchignore`

Drop a `.repocrunchignore` in your repo root. Same syntax as `.gitignore`:

```
docs/
*.log
tests/
*.min.js
```

## Features

- ⚡ **Fast** — Zero-copy I/O (`memmap2`), parallel scanning (`rayon`), sub-second cached runs
- 🧠 **Tree-sitter** — Accurate parsing for Rust, TypeScript, JavaScript, Python, Go, Java, C/C++
- 🔎 **Query Mode** — Natural language file selection with heuristic expansion
- 📊 **Dependency Graph** — Cross-file import resolution
- 🗂️ **Compact Output** — FILE INDEX, blake3 dedup, minimal delimiters
- 💾 **Caching** — blake3 content hashing, 8× speedup on subsequent runs
- 📋 **Clipboard** — `--copy` sends output directly to clipboard
- 🔧 **Inspection** — `stats`, `explain`, `flow` subcommands

## Architecture

```
scanner → language → index → dependency_graph → query → packer
                                ↑
                             cache (blake3)
```

## Performance

| Metric | Value |
|--------|-------|
| First run (15 files) | ~94ms |
| Cached run | ~6ms |
| Token overhead vs XML | **-56%** |

## License

MIT
