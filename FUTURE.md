# Tokemon - Future Features Roadmap

Features deferred from the PoC/MVP, organized by priority.

## High Priority

### HTTP Proxy Mode
Intercept LLM API calls in real-time by setting `BASE_URL` to a local proxy . Captures usage data at the wire level, independent of log file formats.
- Implementation: `hyper` reverse proxy + `tokio` async runtime
- Config: `tokemon proxy --port 8080 --upstream https://api.anthropic.com`

### Provider API Polling
Query provider billing/usage APIs directly for accurate cost data:
- Claude: `api.anthropic.com/v1/organizations/usage`
- OpenAI/Codex: `api.openai.com/v1/usage`
- OpenRouter: `openrouter.ai/api/v1/credits`
- Google AI: Vertex AI billing API

### Real-time File Watching
Use `notify` crate to watch log directories for changes. Incremental O(1) updates instead of full re-parse. Enables live dashboard.

### Data Preservation Cache
Cache parsed data to survive provider log rotation (e.g., Claude Code's 30-day deletion policy). SQLite cache at `~/.cache/tokemon/usage.db`.

## Medium Priority

### Interactive TUI Dashboard
`ratatui`-based terminal UI with:
- Keyboard navigation (vim keys)
- Sparkline charts for usage trends
- Tab-based views (daily/weekly/monthly/by-provider)
- Live-updating stats

### macOS Menu Bar App
SwiftUI `MenuBarExtra` with:
- Quick usage summary in menu dropdown
- Pacemaker budgeting (green/orange/red status)
- Provider toggles
- Click to open detailed view

### Budget Alerts / Pacemaker System
Set daily/weekly/monthly budgets per provider or total. Traffic-light status:
- Green: under 60% of budget
- Orange: 60-90% of budget
- Red: over 90% of budget

### Configuration File
`~/.config/tokemon/config.toml` for:
- Default providers
- Budget thresholds
- Custom provider paths
- Output preferences
- Pricing cache TTL

### MCP Server Integration
Expose usage data as an MCP tool so AI assistants can self-monitor their token consumption. Useful for cost-aware agents.

## Lower Priority

### VS Code Extension
Webview dashboard showing:
- Session cost in status bar
- Per-file token attribution
- Usage charts

### System Tray Icon (Cross-platform)
- Windows/Linux equivalent of menu bar app
- Settings UI for provider configuration
- Budget notifications

### Cloud Dashboard / Cross-machine Sync
Web dashboard aggregating usage across machines. Optional sync via cloud storage or simple server.

### CSV/PDF Export
Export reports for expense reporting and team billing.

### Per-session Breakdown View
`tokemon sessions` command showing individual sessions with:
- Duration, tokens, cost per session
- Session descriptions (from first user message)

### Statusline Output Mode
`tokemon --statusline` for shell prompt integration:
- Compact format: `$12.34 | 1.2M tok`
- Color codes for budget status

### Piebald SQLite Support
Add `rusqlite` dependency to parse Piebald's `app.db` directly.

### Copilot Token Estimation
Use `tiktoken` to estimate token counts from Copilot chat sessions (which don't include token metadata).
