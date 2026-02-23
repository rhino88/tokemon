<p align="center">
  <h1 align="center">tokemon</h1>
  <p align="center">
    an LLM <b>tok</b>en <b>mon</b>itor
  </p>
  <p align="center">
    <a href="https://opensource.org/licenses/MIT"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
    <a href="https://www.rust-lang.org/"><img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-Rust-orange.svg"></a>
    <img alt="16 providers" src="https://img.shields.io/badge/providers-16-green.svg">
  </p>
</p>

---

Unified token usage tracking across all your AI coding tools. tokemon reads local session logs from 16 providers, estimates costs via LiteLLM pricing, and presents daily, weekly, or monthly reports in the terminal or as JSON.

```
╭────────────┬───────────┬───────────┬─────────────┬───────────────┬───────────────┬──────────╮
│ Date       │     Input │    Output │ Cache Write │    Cache Read │  Total Tokens │     Cost │
├────────────┼───────────┼───────────┼─────────────┼───────────────┼───────────────┼──────────┤
│ 2026-02-20 │   363,489 │   151,776 │  25,842,224 │   391,480,542 │   417,838,031 │  $485.59 │
│ 2026-02-21 │   305,260 │   266,882 │  25,424,702 │   551,464,876 │   577,461,720 │  $733.72 │
│ 2026-02-22 │   317,451 │   218,759 │  40,516,730 │   644,994,151 │   686,047,091 │  $877.50 │
│ ...        │           │           │             │               │               │          │
│ TOTAL      │ 8,205,537 │ 1,755,386 │ 294,100,468 │ 4,796,913,334 │ 5,100,974,725 │ $5475.81 │
╰────────────┴───────────┴───────────┴─────────────┴───────────────┴───────────────┴──────────╯
```

## Highlights

- **16 providers** — Claude Code, Codex, Gemini CLI, Amp, OpenCode, Cline, Roo Code, Kilo Code, Copilot, Pi Agent, Kimi, Droid, OpenClaw, Qwen Code, Piebald, Cursor
- **Auto-discovery** — detects which tools are installed and finds their log directories automatically
- **Cost estimation** — LiteLLM pricing database with three-level model name matching
- **SQLite cache** — parsed data is cached for instant repeated runs and survives log rotation
- **Budget pacemaker** — set daily/weekly/monthly spending limits with progress tracking
- **Statusline mode** — compact one-line output for shell prompts and status bars
- **Two display modes** — compact one-row-per-day (default) or detailed per-model breakdown
- **Filtering** — by provider (`-p`), date range (`--since` / `--until`), sort order (`-o`)
- **JSON output** — `--json` for piping to `jq` or downstream tools
- **Parallel parsing** — multi-threaded file processing with [rayon](https://github.com/rayon-rs/rayon)
- **Configurable** — persistent preferences via `~/.config/tokemon/config.toml`
- **Extensible** — adding a new source is ~20 lines of Rust

## Installation

Requires [Rust 1.83+](https://rustup.rs/).

```bash
git clone https://github.com/mm65x/tokemon.git
cd tokemon
cargo build --release

# Optionally install to PATH:
cargo install --path .
```

## Quick Start

```bash
# See which providers are installed
tokemon discover

# Daily usage report (default)
tokemon

# Per-model breakdown view
tokemon -d breakdown

# Monthly report, JSON output
tokemon monthly --json

# Budget overview
tokemon budget

# Statusline for shell prompts
tokemon statusline
# $42.17 | 1.2B tok | 1 provider | today
```

## Usage

```
tokemon [COMMAND] [OPTIONS]

Commands:
  daily        Show daily usage breakdown (default)
  weekly       Show weekly usage summary
  monthly      Show monthly usage summary
  statusline   Compact one-line output for shell prompts (--period today|week|month)
  budget       Show spending vs configured limits
  discover     List auto-detected providers
  init         Generate default config file

Options:
  -d, --display <MODE>    compact (default) or breakdown
  -p, --provider <NAME>   Filter by provider (repeatable)
      --since <DATE>      Start date (YYYY-MM-DD)
      --until <DATE>      End date (YYYY-MM-DD)
      --no-cost           Skip cost calculation
      --offline           Use cached pricing only
      --refresh           Force re-discovery of files
      --reparse           Force re-parse of all files from disk
  -o, --order <ORDER>     asc (default) or desc
      --json              Output as JSON
```

## Configuration

```bash
tokemon init
# Creates ~/.config/tokemon/config.toml
```

```toml
default_command = "daily"
default_format = "table"
breakdown = false
no_cost = false
offline = false
sort_order = "asc"
providers = []

[budget]
daily = 50.0      # $50/day limit
weekly = 250.0    # $250/week limit
monthly = 800.0   # $800/month limit

[columns]
date = true
provider = true
model = true
input = true
output = true
cache_write = true
cache_read = true
total_tokens = true
cost = true
```

CLI flags always override config values.

## Supported Providers

| Provider | Log Location | Format |
|----------|-------------|--------|
| Claude Code | `~/.claude/projects/**/*.jsonl` | JSONL |
| Codex CLI | `~/.codex/sessions/**/*.jsonl` | JSONL |
| Gemini CLI | `~/.gemini/tmp/**/session*.json` | JSON |
| Amp | `~/.local/share/amp/threads/**/*.jsonl` | JSONL |
| OpenCode | `~/.local/share/opencode/storage/message/**/*.json` | JSON |
| Cline | VSCode globalStorage | JSON |
| Roo Code | VSCode globalStorage | JSON |
| Kilo Code | VSCode globalStorage | JSON |
| Copilot | VSCode workspaceStorage | JSON (stub) |
| Cursor | `~/.config/tokscale/cursor-cache/*.csv` | CSV |
| Qwen Code | `~/.qwen/tmp/**/session.json` | JSON |
| Pi Agent | `~/.pi-agent/sessions/**/*.jsonl` | JSONL |
| Kimi | `~/.kimi/sessions/**/*.jsonl` | JSONL |
| Droid | `~/.droid/sessions/**/*.jsonl` | JSONL |
| OpenClaw | `~/.openclaw/sessions/**/*.jsonl` | JSONL |
| Piebald | `~/Library/Application Support/piebald/app.db` | SQLite (stub) |

Adding a new source requires implementing the `Source` trait — see `src/source/jsonl_source.rs` for a template that covers most JSONL-based tools in ~20 lines.

## Development

```bash
make help          # Show available targets
make build         # Build release binary
make test          # Run tests
make lint          # Run clippy
make fmt           # Format code
make ci            # Run all checks (fmt + lint + test)
```

## Architecture

```
src/
├── main.rs              # CLI entry, command dispatch
├── cli.rs               # clap argument definitions
├── config.rs            # TOML config loading and validation
├── types.rs             # Core data types (Record, Report, etc.)
├── error.rs             # Error types
├── cache.rs             # SQLite cache layer
├── pacemaker.rs         # Budget tracking and limits
├── timestamp.rs         # Shared timestamp parsing
├── cost.rs              # LiteLLM cost calculation engine
├── rollup.rs            # Daily/weekly/monthly grouping
├── dedup.rs             # Deduplication logic
├── render.rs            # Table and JSON rendering
├── paths.rs             # Platform-specific path resolution
└── source/
    ├── mod.rs            # Source trait and SourceSet
    ├── jsonl_source.rs   # Generic JSONL source (5 sources use this)
    ├── cline_format.rs   # Shared Cline-format parser (3 sources use this)
    ├── claude_code.rs    # Claude Code parser
    ├── codex.rs          # Codex CLI parser (state machine)
    └── ...               # One file per source
```

## License

MIT
