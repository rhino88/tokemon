# CLAUDE.md

## Build

Rust 1.83+ required. On this machine, `cargo build` must run from **outside the OneDrive-synced directory** (OneDrive filesystem hooks kill build script binaries). Use `~/tmp/` as the build directory:

```bash
# One-time: copy source and build
cp -r . ~/tmp/tokemon-build
cd ~/tmp/tokemon-build
cargo build --release
cp target/release/tokemon ~/.local/bin/tokemon

# Clean up
rm -rf ~/tmp/tokemon-build
```

Alternatively, build inside Docker:

```bash
docker run --rm \
  -v $(pwd):/app -w /app \
  -v /tmp/ca-bundle.pem:/etc/ssl/certs/ca-certificates.crt:ro \
  -e SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
  -e CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt \
  tokemon-dev cargo build --release
```

## Test

```bash
cd ~/tmp/tokemon-build && cargo test
```

## Git

- Remote: `https://github.com/mm65x/tokemon.git` (private)
- Push: `GH_CONFIG_DIR=/tmp/tokemon-gh gh auth token` for HTTPS auth

## Code Conventions

- **New JSONL providers**: Use `GenericJsonlProvider<C>` from `jsonl_provider.rs` — implement `JsonlProviderConfig` (~15 lines)
- **Cline-derived providers**: Use `ClineFormatParser` from `cline_format.rs`
- **Timestamps**: Always use `parse_utils::parse_timestamp()`, never inline
- **Glob patterns**: Use `PathBuf::join("**/*.jsonl").display().to_string()`
- **Errors**: Skip malformed lines with `continue`, warnings to stderr only
- **Pure functions**: Annotate with `#[must_use]`

## Content Policy

- **Never reference other tools by name** in README, comments, commit messages, or documentation. No comparisons, no "inspired by X", no "like Y". tokemon stands on its own.
- File paths that happen to contain third-party tool names (e.g., `~/.config/tokscale/cursor-cache/`) are acceptable since those are factual filesystem locations.
