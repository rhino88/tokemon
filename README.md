# Tokemon

Unified LLM token usage tracking across all AI coding tool providers.

## Features

- **16 providers**: Claude Code, Codex, Gemini CLI, Amp, OpenCode, Cline, Roo Code, Kilo Code, GitHub Copilot, Pi Agent, Kimi, Droid, OpenClaw, Qwen Code, Piebald, Cursor
- **Auto-discovery**: detects which providers are installed on your machine
- **Cost estimation**: LiteLLM pricing database with three-level model matching
- **Flexible reporting**: daily, weekly, monthly aggregation
- **Two display modes**: detailed per-model breakdown or compact compact view
- **Filtering**: by provider (`-p`), date range (`--since`/`--until`)
- **JSON output**: for piping to `jq` or other tools
- **Deduplication**: by message_id:request_id to prevent double-counting
- **Parallel parsing**: uses rayon for multi-threaded file processing
- **Config system**: `~/.config/tokemon/config.toml` for persistent preferences

## Installation

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) (required for building on machines with endpoint protection)
- macOS, Linux, or Windows

### Quick Start

```bash
git clone git@github.com:mm65x/tokemon.git
cd tokemon

# Build (first run builds Docker image + Rust binary)
./tokemon.sh discover

# Daily usage report
./tokemon.sh

# Weekly with cost, offline pricing
./tokemon.sh weekly --offline
```

### Building Manually (without Docker)

If your machine allows running Rust build scripts:

```bash
cargo build --release
./target/release/tokemon discover
```

### Building with Docker (recommended for corporate machines)

The `tokemon.sh` wrapper handles everything:

```bash
# First run: builds Docker image and compiles the binary
./tokemon.sh discover

# Subsequent runs are instant (uses cached binary)
./tokemon.sh daily --since 2026-02-01
```

## Usage

```
tokemon [COMMAND] [OPTIONS]

Commands:
  daily      Show daily usage breakdown (default)
  weekly     Show weekly usage summary
  monthly    Show monthly usage summary
  discover   List auto-detected providers
  init       Generate default config file

Options:
  -b, --breakdown       Per-model breakdown (default)
      --no-breakdown    Compact mode: one row per date
  -p, --provider NAME   Filter by provider (repeatable)
      --since DATE      Show usage from this date (YYYY-MM-DD)
      --until DATE      Show usage until this date (YYYY-MM-DD)
      --no-cost         Skip cost calculation
      --offline         Use cached pricing data only
  -o, --order ORDER     Sort: asc (default) or desc
      --json            Output as JSON
```

### Examples

```bash
# See which providers are installed
tokemon discover

# Daily report with per-model breakdown
tokemon daily

# Compact view — one row per day
tokemon --no-breakdown

# This month only, JSON output
tokemon monthly --since 2026-02-01 --json

# Just Claude Code, no network
tokemon -p claude-code --offline

# Newest first
tokemon -o desc --since 2026-02-15
```

## Configuration

Generate a config file:

```bash
tokemon init
# Creates ~/.config/tokemon/config.toml
```

Example config:

```toml
# Default subcommand: "daily", "weekly", "monthly"
default_command = "daily"

# Default output: "table" or "json"
default_format = "table"

# Show per-model breakdown by default
breakdown = true

# Skip cost calculation
no_cost = false

# Use offline pricing
offline = false

# Default providers (empty = all available)
providers = []

# Sort order: "asc" or "desc"
sort_order = "asc"

# Column visibility
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

| Provider | Data Source | Format |
|----------|-----------|--------|
| Claude Code | `~/.claude/projects/**/*.jsonl` | JSONL |
| Codex CLI | `~/.codex/sessions/**/*.jsonl` | JSONL (state machine) |
| Gemini CLI | `~/.gemini/tmp/**/session*.json` | JSON |
| Amp | `~/.local/share/amp/threads/**/*.jsonl` | JSONL |
| OpenCode | `~/.local/share/opencode/storage/message/**/msg_*.json` | JSON |
| Cline | VSCode globalStorage `saoudrizwan.claude-dev` | JSON |
| Roo Code | VSCode globalStorage `rooveterinaryinc.roo-cline` | JSON |
| Kilo Code | VSCode globalStorage `kilocode.kilo-code` | JSON |
| GitHub Copilot | VSCode workspaceStorage (no token data) | JSON |
| Cursor | `~/.config/tokscale/cursor-cache/usage*.csv` | CSV |
| Qwen Code | `~/.qwen/tmp/**/session.json` | JSON |
| Piebald | `~/Library/Application Support/piebald/app.db` | SQLite (stub) |



