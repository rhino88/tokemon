# CLAUDE.md

## Build

Rust 1.83+ required. Build from the symlinked directory (`~/code/tokenusage`), which avoids OneDrive filesystem hook issues:

```bash
cd ~/code/tokenusage
cargo build --release
cp target/release/tokemon ~/.local/bin/tokemon
```

## Test

```bash
cd ~/code/tokenusage && cargo test
```

## Git

- Remote: `https://github.com/mm65x/tokemon.git` (private)
- Push: `GH_CONFIG_DIR=/tmp/tokemon-gh gh auth token` for HTTPS auth

## Code Conventions

- **New JSONL sources**: Use `JsonlSource<C>` from `jsonl_source.rs` — implement `JsonlSourceConfig` (~15 lines)
- **Cline-derived sources**: Use `ClineFormat` from `cline_format.rs`
- **Timestamps**: Always use `timestamp::parse_timestamp()`, never inline
- **Glob patterns**: Use `PathBuf::join("**/*.jsonl").display().to_string()`
- **Errors**: Skip malformed lines with `continue`, warnings to stderr only
- **Pure functions**: Annotate with `#[must_use]`

## Content Policy

- **Never reference other tools by name** in README, comments, commit messages, or documentation. No comparisons, no "inspired by X", no "like Y". tokemon stands on its own.
- File paths that happen to contain third-party tool names (e.g., `~/.config/tokscale/cursor-cache/`) are acceptable since those are factual filesystem locations.
