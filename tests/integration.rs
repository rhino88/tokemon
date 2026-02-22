use std::path::Path;
use tokemon::provider::Provider;

#[test]
fn test_claude_code_parse_fixture() {
    let provider = tokemon::provider::claude_code::ClaudeCodeProvider::new();
    let path = Path::new("tests/fixtures/claude_sample.jsonl");
    let entries = provider.parse_file(path).unwrap();

    // Should have 3 assistant entries (last one is a duplicate of req_003/msg_003)
    // But parse_file doesn't dedup - that happens in parse_all
    assert_eq!(entries.len(), 4);

    // First entry
    assert_eq!(entries[0].provider, "claude-code");
    assert_eq!(entries[0].model.as_deref(), Some("claude-opus-4-1-20250805"));
    assert_eq!(entries[0].input_tokens, 100);
    assert_eq!(entries[0].output_tokens, 50);
    assert_eq!(entries[0].cache_creation_tokens, 500);
    assert_eq!(entries[0].cache_read_tokens, 0);
    assert_eq!(entries[0].request_id.as_deref(), Some("req_001"));
    assert_eq!(entries[0].message_id.as_deref(), Some("msg_001"));

    // Second entry
    assert_eq!(entries[1].input_tokens, 200);
    assert_eq!(entries[1].output_tokens, 150);
    assert_eq!(entries[1].cache_read_tokens, 400);

    // Third entry - different model, different day
    assert_eq!(entries[2].model.as_deref(), Some("claude-sonnet-4-20250514"));
    assert_eq!(entries[2].input_tokens, 50);
}

#[test]
fn test_claude_code_dedup() {
    let provider = tokemon::provider::claude_code::ClaudeCodeProvider::new();
    let path = Path::new("tests/fixtures/claude_sample.jsonl");
    let entries = provider.parse_file(path).unwrap();

    // Before dedup: 4 entries (duplicate msg_003:req_003)
    assert_eq!(entries.len(), 4);

    let deduped = tokemon::dedup::deduplicate(entries);
    // After dedup: 3 entries (duplicate removed)
    assert_eq!(deduped.len(), 3);
}

#[test]
fn test_codex_parse_fixture() {
    let provider = tokemon::provider::codex::CodexProvider::new();
    let path = Path::new("tests/fixtures/codex_sample.jsonl");
    let entries = provider.parse_file(path).unwrap();

    assert_eq!(entries.len(), 2);

    // First token_count: input=300, cached=50, so actual_input=250
    assert_eq!(entries[0].provider, "codex");
    assert_eq!(entries[0].model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(entries[0].input_tokens, 250); // 300 - 50 cached
    assert_eq!(entries[0].output_tokens, 100);
    assert_eq!(entries[0].cache_read_tokens, 50);

    // Second: input=500, cached=100, actual=400
    assert_eq!(entries[1].input_tokens, 400);
    assert_eq!(entries[1].output_tokens, 200);
    assert_eq!(entries[1].cache_read_tokens, 100);
}

#[test]
fn test_gemini_parse_fixture() {
    let provider = tokemon::provider::gemini::GeminiProvider::new();
    let path = Path::new("tests/fixtures/gemini_sample.json");
    let entries = provider.parse_file(path).unwrap();

    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].provider, "gemini");
    assert_eq!(entries[0].model.as_deref(), Some("gemini-2.5-flash"));
    assert_eq!(entries[0].input_tokens, 150);
    assert_eq!(entries[0].output_tokens, 75);
    assert_eq!(entries[0].cache_read_tokens, 30);
    assert_eq!(entries[0].thinking_tokens, 20);

    assert_eq!(entries[1].input_tokens, 200);
    assert_eq!(entries[1].thinking_tokens, 50);
}

#[test]
fn test_cline_parse_fixture() {
    let provider = tokemon::provider::cline::ClineProvider::new();
    let path = Path::new("tests/fixtures/cline_sample.json");
    let entries = provider.parse_file(path).unwrap();

    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].provider, "cline");
    assert_eq!(entries[0].input_tokens, 500);
    assert_eq!(entries[0].output_tokens, 200);
    assert_eq!(entries[0].cache_creation_tokens, 100);
    assert_eq!(entries[0].cache_read_tokens, 50);
    assert_eq!(entries[0].cost_usd, Some(0.015));

    assert_eq!(entries[1].input_tokens, 800);
    assert_eq!(entries[1].cost_usd, Some(0.025));
}

#[test]
fn test_daily_aggregation() {
    let provider = tokemon::provider::claude_code::ClaudeCodeProvider::new();
    let path = Path::new("tests/fixtures/claude_sample.jsonl");
    let entries = provider.parse_file(path).unwrap();
    let entries = tokemon::dedup::deduplicate(entries);

    let summaries = tokemon::aggregator::aggregate_daily(&entries);

    // Should have 2 days: 2026-02-20 and 2026-02-21
    assert_eq!(summaries.len(), 2);

    // First day: 2 entries (opus model)
    assert_eq!(summaries[0].total_requests, 2);
    assert_eq!(summaries[0].total_input, 300); // 100 + 200

    // Second day: 1 entry (sonnet model)
    assert_eq!(summaries[1].total_requests, 1);
    assert_eq!(summaries[1].total_input, 50);
}

#[test]
fn test_date_filtering() {
    use chrono::NaiveDate;

    let provider = tokemon::provider::claude_code::ClaudeCodeProvider::new();
    let path = Path::new("tests/fixtures/claude_sample.jsonl");
    let entries = provider.parse_file(path).unwrap();

    let since = NaiveDate::from_ymd_opt(2026, 2, 21);
    let filtered = tokemon::aggregator::filter_by_date(entries, since, None);

    // Only entries from Feb 21 should remain
    assert_eq!(filtered.len(), 2); // 2 entries on that day (including duplicate)
    for entry in &filtered {
        assert_eq!(entry.timestamp.date_naive().to_string(), "2026-02-21");
    }
}

#[test]
fn test_usage_entry_total_tokens() {
    use chrono::Utc;
    use tokemon::types::UsageEntry;

    let entry = UsageEntry {
        timestamp: Utc::now(),
        provider: "test".to_string(),
        model: None,
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 30,
        cache_creation_tokens: 20,
        thinking_tokens: 10,
        cost_usd: None,
        message_id: None,
        request_id: None,
        session_id: None,
    };

    assert_eq!(entry.total_tokens(), 210);
}

#[test]
fn test_dedup_key_generation() {
    use chrono::Utc;
    use tokemon::types::UsageEntry;

    let entry_both = UsageEntry {
        timestamp: Utc::now(),
        provider: "test".to_string(),
        model: Some("model-a".to_string()),
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        thinking_tokens: 0,
        cost_usd: None,
        message_id: Some("msg_1".to_string()),
        request_id: Some("req_1".to_string()),
        session_id: None,
    };
    assert_eq!(entry_both.dedup_key(), Some("msg_1:req_1".to_string()));

    let entry_msg_only = UsageEntry {
        message_id: Some("msg_2".to_string()),
        request_id: None,
        ..entry_both.clone()
    };
    assert_eq!(
        entry_msg_only.dedup_key(),
        Some("msg_2:model-a:100:50".to_string())
    );

    let entry_none = UsageEntry {
        message_id: None,
        request_id: None,
        ..entry_both
    };
    assert_eq!(entry_none.dedup_key(), None);
}
