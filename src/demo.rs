//! Hard-coded demo data for showcasing the dashboard during live demos.
//!
//! **TEMPORARY** — remove this module, its `mod demo;` in `main.rs`, and the
//! two `extend(demo::records())` call sites in `tui/app.rs` once the demo is
//! over.
//!
//! Produces deterministic synthetic usage for three models across three
//! providers so the spike chart and contribution heatmap have something to
//! render on a fresh machine.

use std::borrow::Cow;
use std::sync::OnceLock;

use chrono::{DateTime, Duration, Utc};

use crate::types::Record;

/// Frozen "now" for the demo — captured on the first call to [`records`] and
/// reused forever afterwards. Without this, every reload re-anchors records
/// to a fresh `Utc::now()` and the spike chart's past buckets appear to drift
/// forward as wall-clock time advances.
static DEMO_NOW: OnceLock<DateTime<Utc>> = OnceLock::new();

fn demo_now() -> DateTime<Utc> {
    *DEMO_NOW.get_or_init(Utc::now)
}

/// Single toggle to disable demo injection without deleting code.
const DEMO_ENABLED: bool = false;

/// Number of days of back-history to synthesize for the heatmap.
const HISTORY_DAYS: i64 = 365;

/// Number of synthetic records scattered across today for the spike chart.
const TODAY_BURST_COUNT: usize = 48;

/// Number of records clustered in the last hour to simulate an active
/// coding session. At 480 records the rightmost area of the chart averages
/// 2–3 records per 3-second bucket, giving the spike chart enough variation
/// in bucket totals to render real spike-shape contrast.
const ACTIVE_SESSION_COUNT: usize = 480;

/// Window in seconds for the "active session" cluster (last hour).
const ACTIVE_SESSION_WINDOW_SECS: i64 = 3_600;

struct DemoModel {
    provider: &'static str,
    model: &'static str,
    base_input: u64,
    base_output: u64,
    base_cache_read: u64,
    base_cache_creation: u64,
}

const DEMO_MODELS: [DemoModel; 3] = [
    DemoModel {
        provider: "claude-code",
        model: "claude-sonnet-4-5-20250929",
        base_input: 1_500,
        base_output: 3_200,
        base_cache_read: 18_000,
        base_cache_creation: 4_500,
    },
    DemoModel {
        provider: "codex",
        model: "gpt-5",
        base_input: 4_200,
        base_output: 2_600,
        base_cache_read: 11_000,
        base_cache_creation: 0,
    },
    DemoModel {
        provider: "gemini",
        model: "gemini-2.5-pro",
        base_input: 6_400,
        base_output: 1_900,
        base_cache_read: 0,
        base_cache_creation: 0,
    },
];

/// Generate the synthetic demo records. Deterministic — same output every
/// call, so repeat injection on cache reloads does not drift.
#[must_use]
pub fn records() -> Vec<Record> {
    if !DEMO_ENABLED {
        return Vec::new();
    }

    let now = demo_now();
    let mut rng = XorShift::new(0x1234_5678_9abc_def0);
    let mut out = Vec::with_capacity(1024);

    // ── 365-day history (drives the heatmap) ────────────────────────────
    //
    // Each day picks one dominant model (the one that drives the heatmap
    // color for that cell), plus an optional small sprinkle of a second
    // provider so the table has variety. Distribution:
    //   - Anthropic dominant: ~62% of active days (orange — the main pattern)
    //   - OpenAI    dominant: ~18% overall, heavily biased to the last 30 days
    //                         so the heatmap has recent green cells
    //   - Google    dominant: ~8%  (blue, sprinkled throughout the year)
    //   - Empty day: ~12%
    //
    // "Spike" days are rolled once per ~8 days and give a 3–6× multiplier,
    // pushing them into the top heatmap intensity bucket.
    for days_ago in 0..HISTORY_DAYS {
        // Empty-cell roll. ~12% of days are completely empty.
        if rng.next() % 100 < 12 {
            continue;
        }

        let day = now - Duration::days(days_ago);

        // Spike day? Roughly every 8 days, with a high multiplier.
        let is_spike = rng.next().is_multiple_of(8);
        let day_mult = if is_spike {
            3.0 + (rng.next() % 35) as f64 / 10.0 // 3.0 – 6.5x
        } else {
            0.55 + (rng.next() % 140) as f64 / 100.0 // 0.55 – 1.95x
        };

        // Pick dominant model. The last 30 days are weighted more toward
        // OpenAI to show "recent codex usage"; otherwise the default mix.
        let roll = rng.next() % 100;
        let dominant_idx: usize = if days_ago < 30 {
            // Recent window: 45% Claude, 40% Codex, 12% Gemini, 3% mixed.
            match roll {
                0..=44 => 0,                     // claude-sonnet
                45..=84 => 1,                    // gpt-5
                85..=96 => 2,                    // gemini-2.5-pro
                _ => (rng.next() % 3) as usize,  // small chaos
            }
        } else {
            // Normal window: 70% Claude, 20% Codex, 10% Gemini.
            match roll {
                0..=69 => 0,  // claude-sonnet (orange dominates the calendar)
                70..=89 => 1, // gpt-5
                _ => 2,       // gemini-2.5-pro
            }
        };

        let dominant = &DEMO_MODELS[dominant_idx];

        // 2–6 records for the dominant model, scaled up on spike days.
        let record_count = if is_spike {
            4 + (rng.next() % 5) // 4–8
        } else {
            2 + (rng.next() % 4) // 2–5
        };
        for i in 0..record_count {
            let ts = day_offset(day, &mut rng, i);
            let jitter = 0.65 + (rng.next() % 110) as f64 / 100.0;
            out.push(build_record(dominant, ts, day_mult * jitter, "demo-history"));
        }

        // Optional small sprinkle of a second provider — small enough not
        // to overtake the dominant model's cost, but enough to show up in
        // the usage table and to add a whisper of blue/green under orange.
        // ~35% of days get a sprinkle.
        if rng.next() % 100 < 35 {
            let mut sprinkle_idx = (rng.next() % 3) as usize;
            if sprinkle_idx == dominant_idx {
                sprinkle_idx = (sprinkle_idx + 1) % 3;
            }
            let sprinkle = &DEMO_MODELS[sprinkle_idx];
            let count = 1 + (rng.next() % 2); // 1–2 records
            for i in 0..count {
                let ts = day_offset(day, &mut rng, i + 10);
                // Much smaller scale so it doesn't take over the color.
                let small_scale = 0.15 + (rng.next() % 20) as f64 / 100.0;
                out.push(build_record(sprinkle, ts, small_scale, "demo-history"));
            }
        }
    }

    // ── Today's cluster (drives the spike chart) ────────────────────────
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight always valid")
        .and_utc();
    let elapsed_secs = (now - today_start).num_seconds().max(60);

    for k in 0..TODAY_BURST_COUNT {
        // Spread records across the elapsed day, plus ±20s jitter.
        let base_offset = (k as i64 * elapsed_secs) / TODAY_BURST_COUNT as i64;
        let jitter_secs = (rng.next() % 41) as i64 - 20;
        let offset = (base_offset + jitter_secs).clamp(0, elapsed_secs - 1);
        let ts = today_start + Duration::seconds(offset);

        let m = &DEMO_MODELS[(rng.next() % DEMO_MODELS.len() as u64) as usize];
        let scale = 0.4 + (rng.next() % 160) as f64 / 100.0;
        out.push(build_record(m, ts, scale, "demo-today"));
    }

    // ── Active session in the last hour (dense spike chart cluster) ─────
    // Density ramps up toward "now" to look like an in-progress coding
    // session — denser and larger records near the present.
    let window = ACTIVE_SESSION_WINDOW_SECS.min(elapsed_secs);
    for k in 0..ACTIVE_SESSION_COUNT {
        // Quadratic bias: records clump closer to `now`. `t` is 0..1 where
        // 1 == present; squaring skews most records toward the right side.
        let t = (k as f64) / (ACTIVE_SESSION_COUNT as f64);
        let biased = t * t;
        let secs_before_now = ((1.0 - biased) * window as f64) as i64;
        // ±8s jitter so buckets aren't perfectly uniform.
        let jitter = (rng.next() % 17) as i64 - 8;
        let secs_before_now = (secs_before_now + jitter).max(0);
        let ts = now - Duration::seconds(secs_before_now);

        // Bias toward Claude Sonnet (primary driver) with gpt-5 and
        // gemini mixed in — looks like a real multi-agent session.
        let m = match rng.next() % 10 {
            0 | 1 => &DEMO_MODELS[1], // gpt-5
            2 => &DEMO_MODELS[2],     // gemini-2.5-pro
            _ => &DEMO_MODELS[0],     // claude-sonnet-4-5 (dominant)
        };
        // Larger scale during the active session — this is "real work".
        let scale = 0.8 + (rng.next() % 220) as f64 / 100.0;
        out.push(build_record(m, ts, scale, "demo-active"));
    }

    // Guarantee at least one record within the heat-glow window so the
    // spike chart's glow animation is live during the demo.
    out.push(build_record(
        &DEMO_MODELS[0],
        now - Duration::seconds(3),
        2.2,
        "demo-active",
    ));

    out
}

/// Compute a timestamp within `day`, shifted back by a random few hours/minutes,
/// with a unique millisecond offset per `i` so records never collide on dedup.
fn day_offset(day: chrono::DateTime<Utc>, rng: &mut XorShift, i: u64) -> chrono::DateTime<Utc> {
    day - Duration::hours((rng.next() % 14) as i64)
        - Duration::minutes((rng.next() % 60) as i64)
        - Duration::seconds((rng.next() % 60) as i64)
        - Duration::milliseconds(i as i64 * 37)
}

fn build_record(
    m: &DemoModel,
    ts: chrono::DateTime<Utc>,
    scale: f64,
    session_prefix: &str,
) -> Record {
    Record {
        timestamp: ts,
        provider: Cow::Borrowed(m.provider),
        model: Some(m.model.to_string()),
        input_tokens: (m.base_input as f64 * scale) as u64,
        output_tokens: (m.base_output as f64 * scale) as u64,
        cache_read_tokens: (m.base_cache_read as f64 * scale) as u64,
        cache_creation_tokens: (m.base_cache_creation as f64 * scale) as u64,
        thinking_tokens: 0,
        cost_usd: None,
        message_id: None,
        request_id: None,
        session_id: Some(format!("{session_prefix}-{}", m.provider)),
    }
}

/// Tiny deterministic PRNG — avoids pulling in the `rand` crate for demo data.
struct XorShift {
    state: u64,
}

impl XorShift {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}
