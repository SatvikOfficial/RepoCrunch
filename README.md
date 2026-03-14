# RepoCrunch 🚀

A high-performance repository packer and query engine written in Rust, designed to prepare codebases for Large Language Models (LLMs).

It recursively scans your repository, builds an intelligent file index utilizing **Tree-sitter** for parsing, and packs relevant source code into an LLM-native text format.

## Features

- **Extremely Fast:** Zero-copy I/O with `memmap2`, SIMD-accelerated search, and threaded parallel walking
- **Smart Caching:** Avoids re-parsing the entire repository on every run utilizing `blake3` content hashing (up to 8× speedups on cached runs)
- **Intelligent Packing:** Tree-sitter accurate import and symbol extraction
- **Query Mode:** Natural-language heuristic expansion (e.g. `repocrunch "auth files"`) and dependency awareness to ensure proper context is packed
- **Dependency Graph:** Understands cross-file boundaries to resolve missing contexts
- **AI-Optimized Output:** Removes XML-like tags mimicking HTML in favor of native token-efficient text with dependency structure exposed

---

## Installation

### Prerequisites

Ensure you have Rust installed. Install it from [rustup.rs](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/repocrunch.git
cd repocrunch

# Build release binary
cargo build --release

# Install to ~/.cargo/bin/
cp target/release/repocrunch ~/.cargo/bin/

# Verify installation
repocrunch --version
```

### Direct Run (without installation)

```bash
# Run directly using cargo
cargo run --release -- [OPTIONS]
```

---

## Usage

Run `repocrunch` anywhere inside a Git repository. It will automatically walk up to find the `.git` root.

### Basic Commands

```bash
# Full repository pack
repocrunch

# Query filtered pack (e.g., only auth-related files and their dependencies)
repocrunch "components relating to login and auth"
```

The tool generates `repocrunch-output.txt` by default in the current directory.

---

## Command-Line Options

```
Usage: repocrunch [OPTIONS] [QUERY]

Arguments:
  [QUERY]  Natural-language query to filter files (e.g. "files related to auth")

Options:
  -o, --output <OUTPUT>      Output file path [default: repocrunch-output.txt]
  -t, --max-tokens <TOKENS>  Maximum token budget
  -f, --format <FORMAT>      Output format: default or ai [default: default]
      --max-file-size <SIZE> Maximum file size in bytes (defaults to 1MB)
  -i, --include <PATTERN>    Additional include glob patterns
  -e, --exclude <PATTERN>    Additional exclude glob patterns
      --json-index           Print the parsed dependency index as JSON
      --copy                 Copy output to clipboard
      --no-cache             Disable caching
  -v, --verbose              Show verbose output
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

```bash
# Pack into the highly compact @file/@lang/@end AI format
repocrunch --format ai

# Enforce a strict token limit (e.g., packing for models with 100K context)
repocrunch --max-tokens 100000

# Copy the packed output directly to your clipboard
repocrunch --copy

# Export the parsed dependency and file index for other tools
repocrunch --json-index > index.json

# Include additional patterns
repocrunch --include "*.md" --include "*.txt"

# Exclude specific patterns
repocrunch --exclude "tests/*" --exclude "*.test.js"

# Verbose output with custom output file
repocrunch -v -o packed-repo.txt

# Query with custom max file size
repocrunch "authentication" --max-file-size 5000000
```

---

## Subcommands

### `stats` - Repository Statistics

View deep repository statistics and language breakdown:

```bash
repocrunch stats
```

### `explain` - File Explanation

Explain a specific file's exported symbols and inbound/outbound imports:

```bash
repocrunch explain src/main.rs
```

### `flow` - Execution Flow Trace

Map out the path of execution or imports across the codebase:

```bash
repocrunch flow <symbol-name>
```

---

## Configuration

### `.repocrunchignore`

By default, RepoCrunch:
- Skips binary files (detected automatically)
- Skips hidden files
- Skips files over 1MB
- Respects your `.gitignore`

You can create a `.repocrunchignore` file in your repository root to ignore specific paths specifically for packing. Format is the same as `.gitignore`.

### Override File Size Limit

```bash
repocrunch --max-file-size 5000000  # 5 MB
```

---

## Output Formats

### Default Format

Human-readable format with file paths and content:

```
File: src/main.rs
Language: Rust
Tokens: 1234

[File content here]
```

### AI Format (`--format ai`)

Compact format optimized for LLM context windows:

```
@file: src/main.rs
@lang: rust
@tokens: 1234

[File content here]
@end
```

---

## Development

```bash
# Run in debug mode
cargo run -- [OPTIONS]

# Run tests
cargo test

# Check for errors
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

---

## License

MIT License - See LICENSE file for details.

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

**Made with ❤️ using Rust**
