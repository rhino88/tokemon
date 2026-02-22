# CLAUDE.md - Tokemon Development Guide

## Build Environment

**IMPORTANT**: This machine has IT endpoint protection that blocks Rust build-script-build binaries. All `cargo` commands MUST be run inside Docker.

### Running cargo commands

Never run `cargo build`, `cargo test`, `cargo check`, or `cargo clippy` directly. Always use Docker:

```bash
# Build
docker run --rm \
  -v /Users/mm725821/Documents/code/tokenusage:/app \
  -v /tmp/ca-bundle.pem:/etc/ssl/certs/ca-certificates.crt:ro \
  -e SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
  -e CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt \
  -w /app tokemon-dev cargo build --release

# Test
docker run --rm \
  -v /Users/mm725821/Documents/code/tokenusage:/app \
  -v /tmp/ca-bundle.pem:/etc/ssl/certs/ca-certificates.crt:ro \
  -e SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
  -e CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt \
  -w /app tokemon-dev cargo test

# Run the binary (mount ~/.claude for real data)
docker run --rm \
  -v /Users/mm725821/Documents/code/tokenusage:/app \
  -v /Users/mm725821/.claude:/root/.claude:ro \
  -v /Users/mm725821/.cache/tokemon:/root/.cache/tokemon:ro \
  -w /app tokemon-dev ./target/release/tokemon [ARGS]
```

Or use the wrapper script: `./tokemon.sh [ARGS]`

### Docker image

The `tokemon-dev` image is based on `rust:1.83-slim`. If it doesn't exist, build it:

```bash
docker build -t tokemon-dev .
```

### CA certificates

Corporate SSL inspection intercepts HTTPS traffic. The CA bundle at `/tmp/ca-bundle.pem` must be mounted into the container for `cargo` to fetch crates. Regenerate it if needed:

```bash
security export -t certs -f pemseq -k /Library/Keychains/System.keychain -o /tmp/tokemon_system_certs.pem
security export -t certs -f pemseq -k /System/Library/Keychains/SystemRootCertificates.keychain -o /tmp/tokemon_root_certs.pem
cat /tmp/tokemon_system_certs.pem /tmp/tokemon_root_certs.pem > /tmp/ca-bundle.pem
```

## Project Structure

```
src/
├── main.rs              # Entry point, command dispatch
├── lib.rs               # Library root
├── cli.rs               # clap CLI with DisplayMode enum
├── config.rs            # TOML config (~/.config/tokemon/config.toml)
├── types.rs             # UsageEntry, ModelUsage, DailySummary, Report
├── error.rs             # TokemonError enum
├── parse_utils.rs       # Shared timestamp parsing
├── pricing.rs           # LiteLLM-based cost calculation
├── aggregator.rs        # Daily/weekly/monthly grouping
├── dedup.rs             # Deduplication by message_id:request_id
├── output.rs            # Table (breakdown/compact) and JSON output
├── paths.rs             # Platform-specific path resolution
└── provider/
    ├── mod.rs            # Provider trait + ProviderRegistry
    ├── jsonl_provider.rs # Generic JSONL provider (used by 5 providers)
    ├── claude_code.rs    # Claude Code JSONL parser
    ├── codex.rs          # Codex CLI JSONL state machine parser
    ├── gemini.rs         # Gemini CLI JSON parser
    ├── cline_format.rs   # Shared Cline-format parser (used by 3 providers)
    ├── cline.rs          # Cline (via cline_format)
    ├── roo_code.rs       # Roo Code (via cline_format)
    ├── kilo_code.rs      # Kilo Code (via cline_format)
    ├── opencode.rs       # OpenCode JSON parser
    ├── amp.rs            # Amp (via jsonl_provider)
    ├── kimi.rs           # Kimi (via jsonl_provider)
    ├── droid.rs          # Droid (via jsonl_provider)
    ├── openclaw.rs       # OpenClaw (via jsonl_provider)
    ├── pi_agent.rs       # Pi Agent (via jsonl_provider)
    ├── qwen.rs           # Qwen Code JSON parser
    ├── copilot.rs        # GitHub Copilot (stub - no token data)
    ├── piebald.rs        # Piebald (stub - needs rusqlite)
    └── cursor.rs         # Cursor CSV parser
```

## Git

- Remote: `https://github.com/mm65x/tokemon.git` (private)
- Account: mm65x@users.noreply.github.com
- Push requires: `GH_CONFIG_DIR=/tmp/tokemon-gh gh auth token` for HTTPS auth

## Code Style

- Shared parsing patterns: Use `jsonl_provider.rs` (GenericJsonlProvider<C>) for simple JSONL providers, `cline_format.rs` for Cline-derived formats
- Timestamps: Always use `parse_utils::parse_timestamp()` - never inline timestamp parsing
- Glob patterns: Use `PathBuf::join("**/*.jsonl").display().to_string()` not `format!()`
- Error handling: Skip malformed lines with `continue`, warn to stderr (not stdout)
- Pure functions: Annotate with `#[must_use]`
