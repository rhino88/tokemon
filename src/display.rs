//! Display name translation for clients, models, and API providers.
//!
//! Keeps all presentation logic separate from rendering layout.

/// Map a raw client identifier (source name) to a human-readable display name.
/// "claude-code" -> "Claude Code", "roo-code" -> "Roo Code", etc.
/// Unknown identifiers are returned as-is with title case.
#[must_use]
pub fn display_client(raw: &str) -> String {
    match raw {
        "claude-code" => "Claude Code".into(),
        "codex" => "Codex CLI".into(),
        "gemini" => "Gemini CLI".into(),
        "opencode" => "OpenCode".into(),
        "amp" => "Amp".into(),
        "cline" => "Cline".into(),
        "roo-code" => "Roo Code".into(),
        "kilo-code" => "Kilo Code".into(),
        "copilot" => "GitHub Copilot".into(),
        "pi-agent" => "Pi Agent".into(),
        "kimi" => "Kimi".into(),
        "droid" => "Droid".into(),
        "openclaw" => "OpenClaw".into(),
        "qwen" => "Qwen Code".into(),
        "piebald" => "Piebald".into(),
        "cursor" => "Cursor".into(),
        other => title_case(other),
    }
}

/// Normalize a raw model name for display.
/// Strips provider prefixes (vertexai., openai/, anthropic/, etc.),
/// the `claude-` prefix, and date suffixes (-YYYYMMDD).
///
/// "claude-opus-4-6-20250805" -> "opus-4-6"
/// "vertexai.gemini-2.5-flash" -> "gemini-2.5-flash"
/// "openai/gpt-4o" -> "gpt-4o"
#[must_use]
pub fn display_model(raw: &str) -> String {
    // Strip @... deployment suffix (e.g., "claude-opus-4-6@default" → "claude-opus-4-6")
    let raw = raw.split('@').next().unwrap_or(raw);

    // Strip slash-based prefixes first (e.g., "bedrock/", "openai/")
    let after_slash = raw.split('/').next_back().unwrap_or(raw);

    // Then strip dot-based prefixes (e.g., "vertexai.", "anthropic.")
    let s = after_slash
        .strip_prefix("vertexai.")
        .or_else(|| after_slash.strip_prefix("anthropic."))
        .unwrap_or(after_slash);

    if let Some(rest) = s.strip_prefix("claude-") {
        return strip_date_suffix(rest).to_string();
    }

    strip_date_suffix(s).to_string()
}

/// Infer the API provider from the raw model name.
/// Uses explicit prefixes first, then falls back to model name patterns.
///
/// "vertexai.gemini-2.5-flash" -> "Vertex AI"
/// "claude-opus-4-1" -> "Anthropic"
/// "gpt-4o" -> "OpenAI"
#[must_use]
pub fn infer_api_provider(raw_model: &str) -> String {
    // Explicit provider prefixes
    if raw_model.starts_with("vertexai.") {
        return "Vertex AI".into();
    }
    if raw_model.starts_with("openai/") {
        return "OpenAI".into();
    }
    if raw_model.starts_with("anthropic/") {
        return "Anthropic".into();
    }
    if raw_model.starts_with("google/") {
        return "Google".into();
    }
    if raw_model.starts_with("bedrock/") || raw_model.starts_with("amazon.") {
        return "AWS Bedrock".into();
    }
    if raw_model.starts_with("azure/") {
        return "Azure".into();
    }
    if raw_model.starts_with("mistral/") {
        return "Mistral".into();
    }

    // Strip any remaining slash-prefix for pattern matching
    let model = raw_model.split('/').next_back().unwrap_or(raw_model);

    // Infer from model name patterns
    if model.starts_with("claude-") {
        return "Anthropic".into();
    }
    if model.starts_with("gemini-") || model.starts_with("gemma-") {
        return "Google".into();
    }
    if model.starts_with("gpt-")
        || model_matches_prefix(model, "o1")
        || model_matches_prefix(model, "o3")
        || model_matches_prefix(model, "o4")
    {
        return "OpenAI".into();
    }
    if model.starts_with("qwen") {
        return "Alibaba".into();
    }
    if model.starts_with("deepseek") {
        return "DeepSeek".into();
    }
    if model.starts_with("mistral") || model.starts_with("codestral") {
        return "Mistral".into();
    }
    if model.contains("llama") {
        return "Meta".into();
    }

    String::new()
}

/// Check if model matches a prefix at a word boundary (exact match or followed by `-`).
fn model_matches_prefix(model: &str, prefix: &str) -> bool {
    model == prefix
        || model
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('-'))
}

fn strip_date_suffix(s: &str) -> &str {
    if s.len() >= 9 {
        let last_9 = &s[s.len() - 9..];
        if last_9.starts_with('-')
            && last_9[1..].len() == 8
            && last_9[1..].chars().all(|c| c.is_ascii_digit())
        {
            return &s[..s.len() - 9];
        }
    }
    s
}

fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let mut result = c.to_uppercase().to_string();
                    result.extend(chars);
                    result
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_client() {
        assert_eq!(display_client("claude-code"), "Claude Code");
        assert_eq!(display_client("roo-code"), "Roo Code");
        assert_eq!(display_client("opencode"), "OpenCode");
        assert_eq!(display_client("copilot"), "GitHub Copilot");
        // Unknown gets title-cased
        assert_eq!(display_client("my-tool"), "My Tool");
    }

    #[test]
    fn test_display_model() {
        assert_eq!(display_model("claude-opus-4-1-20250805"), "opus-4-1");
        assert_eq!(display_model("claude-sonnet-4-20250514"), "sonnet-4");
        assert_eq!(display_model("claude-opus-4-5-20251101"), "opus-4-5");
        assert_eq!(display_model("gpt-5-codex"), "gpt-5-codex");
        assert_eq!(display_model("gemini-2.5-flash"), "gemini-2.5-flash");
        assert_eq!(
            display_model("vertexai.gemini-2.5-flash"),
            "gemini-2.5-flash"
        );
        assert_eq!(display_model("openai/gpt-4o"), "gpt-4o");
        assert_eq!(
            display_model("anthropic/claude-sonnet-4-20250514"),
            "sonnet-4"
        );
        // Double prefix: bedrock/anthropic.claude-*
        assert_eq!(
            display_model("bedrock/anthropic.claude-3-5-sonnet-20241022"),
            "3-5-sonnet"
        );
        assert_eq!(
            display_model("bedrock/anthropic.claude-opus-4-1-20250805"),
            "opus-4-1"
        );
        // @ suffix stripping (OpenCode deployment notation)
        assert_eq!(display_model("claude-opus-4-6@default"), "opus-4-6");
        assert_eq!(
            display_model("vertexai.claude-opus-4-6@default"),
            "opus-4-6"
        );
    }

    #[test]
    fn test_infer_api_provider() {
        assert_eq!(infer_api_provider("vertexai.gemini-2.5-flash"), "Vertex AI");
        assert_eq!(infer_api_provider("openai/gpt-4o"), "OpenAI");
        assert_eq!(infer_api_provider("anthropic/claude-sonnet-4"), "Anthropic");
        assert_eq!(infer_api_provider("claude-opus-4-1-20250805"), "Anthropic");
        assert_eq!(infer_api_provider("gemini-2.5-flash"), "Google");
        assert_eq!(infer_api_provider("gpt-4o"), "OpenAI");
        assert_eq!(infer_api_provider("o1-mini"), "OpenAI");
        assert_eq!(infer_api_provider("deepseek-v3"), "DeepSeek");
        assert_eq!(infer_api_provider("qwen-2.5-coder"), "Alibaba");
        assert_eq!(infer_api_provider("unknown-model"), "");
        // Vertex AI detection via model prefix (Claude Code msg_vrtx_ detection)
        assert_eq!(infer_api_provider("vertexai.claude-opus-4-6"), "Vertex AI");
    }
}
