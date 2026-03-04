# CLAUDE.md

## Build & Test

Build from the symlinked directory (`~/code/tokenusage`), which avoids OneDrive filesystem hook issues:

```bash
cargo build --release            # debug: cargo build
cargo test                       # run all tests
cp target/release/tokemon ~/.local/bin/tokemon
```

## Lint

```bash
cargo clippy -- -D warnings
cargo fmt -- --check
```

## Git

- Remote: `https://github.com/mm65x/tokemon.git` (private)
- Push: `GH_CONFIG_DIR=/tmp/tokemon-gh gh auth token` for HTTPS auth

## Code Conventions

- **New JSONL sources**: Implement `JsonlSourceConfig` (~15 lines) and use `JsonlSource<C>` from `source/jsonl_source.rs`
- **Cline-derived sources**: Use `ClineFormat` from `source/cline_format.rs`
- **Timestamps**: Always use `timestamp::parse_timestamp()`, never inline parsing
- **File discovery**: Each `Source` implements `discover_files()` using helpers from `source/discover.rs` (`collect_by_ext`, `walk_by_ext`). No glob crate — use bounded `read_dir` walking only.
- **Display names**: Use `display.rs` functions (`display_client`, `display_model`, `infer_api_provider`) for presentation
- **Errors**: Skip malformed lines with `continue`, warnings to stderr only
- **Pure functions**: Annotate with `#[must_use]`
- **Pre-filtering**: JSONL parsers should `line.contains()` check before `serde_json::from_str` to skip non-matching lines cheaply
- **BufReader**: Use `BufReader::with_capacity(64 * 1024, file)` for line-by-line parsers

## Content Policy

- **Never reference other tools by name** in README, comments, commit messages, or documentation. No comparisons, no "inspired by X", no "like Y". tokemon stands on its own.
- File paths that contain third-party tool names (e.g., `~/.config/tokscale/cursor-cache/`) are acceptable since those are factual filesystem locations.
