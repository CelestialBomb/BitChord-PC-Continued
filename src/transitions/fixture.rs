//! Runs the JSON fixtures in docs/transitions-fixtures against the Rust planner.
//! The same files are recorded from the Android planner, so a mismatch here
//! means the two apps disagree about a transition rule.

use super::{BeatAwareConfig, TransitionSettings, plan, plan_transition};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct FixtureAnalysis {
    duration: f64,
    bpm: f64,
    #[serde(rename = "beatConfidence")]
    beat_confidence: f64,
    key: String,
    #[serde(rename = "keyConfidence")]
    key_confidence: f64,
    vocal: f64,
    #[serde(rename = "firstBeat")]
    first_beat: f64,
    #[serde(rename = "contentEndTime")]
    content_end: f64,
    status: String,
}

impl Default for FixtureAnalysis {
    fn default() -> Self {
        Self {
            duration: 200.0,
            bpm: 120.0,
            beat_confidence: 0.9,
            key: String::new(),
            key_confidence: 0.0,
            vocal: 0.0,
            first_beat: 0.5,
            content_end: 0.0,
            status: "ready".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct FixtureTrack {
    id: String,
    #[serde(rename = "durationMs")]
    duration_ms: u64,
    title: String,
    artist: String,
    album: String,
    #[serde(rename = "albumId")]
    album_id: String,
    analysis: FixtureAnalysis,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct FixtureSettings {
    automix: bool,
    #[serde(rename = "beatAware")]
    beat_aware: bool,
    #[serde(rename = "phraseAligned")]
    phrase_aligned: bool,
    #[serde(rename = "tempoMatching")]
    tempo_matching: bool,
    #[serde(rename = "albumGapless")]
    album_gapless: bool,
    #[serde(rename = "fallbackFadeSeconds")]
    fallback_fade_seconds: f64,
}

impl Default for FixtureSettings {
    fn default() -> Self {
        Self {
            automix: true,
            beat_aware: false,
            phrase_aligned: false,
            tempo_matching: true,
            album_gapless: false,
            fallback_fade_seconds: 8.0,
        }
    }
}

fn one() -> f64 {
    1.0
}

#[derive(Deserialize)]
#[serde(default)]
struct Expected {
    reason: String,
    blocked: bool,
    #[serde(rename = "transitionStyle")]
    style: String,
    #[serde(rename = "transitionStartSeconds")]
    start: f64,
    #[serde(rename = "fadeSeconds")]
    fade: f64,
    #[serde(rename = "incomingCueSeconds")]
    cue: f64,
    #[serde(rename = "incomingPlaybackRate")]
    incoming_rate: f64,
    #[serde(rename = "outgoingPlaybackRate")]
    outgoing_rate: f64,
    #[serde(rename = "policyReasons")]
    policy_reasons: Vec<String>,
}

impl Default for Expected {
    fn default() -> Self {
        Self {
            reason: String::new(),
            blocked: false,
            style: "EQUAL_POWER".into(),
            start: 0.0,
            fade: 0.0,
            cue: 0.0,
            incoming_rate: one(),
            outgoing_rate: one(),
            policy_reasons: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct FixtureCase {
    name: String,
    settings: FixtureSettings,
    outgoing: FixtureTrack,
    incoming: FixtureTrack,
    #[serde(rename = "positionSeconds")]
    position: f64,
    expected: Expected,
    #[serde(rename = "androidOnly")]
    android_only: bool,
}

impl Default for FixtureCase {
    fn default() -> Self {
        Self {
            name: String::new(),
            settings: FixtureSettings::default(),
            outgoing: FixtureTrack::default(),
            incoming: FixtureTrack::default(),
            position: 10.0,
            expected: Expected::default(),
            android_only: false,
        }
    }
}

fn expand(track: &FixtureTrack) -> plan::Track {
    let a = &track.analysis;
    let beat = if a.bpm > 0.0 { 60.0 / a.bpm } else { 0.0 };
    let mut downbeats = Vec::new();
    if beat > 0.0 {
        let mut t = a.first_beat;
        while t < a.duration - 1.0 {
            downbeats.push(t);
            t += 4.0 * beat;
        }
    }
    let phrases = downbeats.iter().copied().enumerate().filter(|(i, _)| i % 8 == 0).map(|(_, t)| t).collect();
    plan::Track {
        id: track.id.clone(),
        duration_ms: track.duration_ms,
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        album_id: track.album_id.clone(),
        analysis: Some(plan::Analysis {
            id: track.id.clone(),
            status: a.status.clone(),
            duration: a.duration,
            bpm: a.bpm,
            beat_confidence: a.beat_confidence,
            beat_interval: beat,
            phrase_boundaries: phrases,
            downbeats,
            first_beat: a.first_beat,
            key: a.key.clone(),
            key_confidence: a.key_confidence,
            audible_start: a.first_beat,
            content_end: if a.content_end == 0.0 { a.duration - 2.0 } else { a.content_end },
            energy: (0..(a.duration * 2.0) as usize)
                .map(|i| plan::EnergySample { time: i as f64 / 2.0, energy: 1.0 })
                .collect(),
            vocal_probability: a.vocal,
            ..Default::default()
        }),
    }
}

#[test]
fn pc_planner_matches_android_recorded_fixtures() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/transitions-fixtures");
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut failures = Vec::new();
    for path in files {
        let case: FixtureCase = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        if case.android_only {
            continue;
        }
        let settings = TransitionSettings {
            automix: case.settings.automix,
            config: BeatAwareConfig {
                enabled: case.settings.beat_aware,
                phrase_aligned: case.settings.phrase_aligned,
                tempo_matching: case.settings.tempo_matching,
                album_gapless: case.settings.album_gapless,
            },
            fallback_fade: Duration::from_secs_f64(case.settings.fallback_fade_seconds),
        };
        let actual = plan_transition(
            &expand(&case.outgoing),
            &expand(&case.incoming),
            Duration::from_secs_f64(case.position),
            &settings,
        );
        let e = case.expected;
        let mut diffs = Vec::new();
        if actual.blocked != e.blocked { diffs.push(format!("blocked {} != {}", actual.blocked, e.blocked)); }
        if actual.reason != e.reason { diffs.push(format!("reason {} != {}", actual.reason, e.reason)); }
        if actual.style.as_str() != e.style { diffs.push(format!("style {} != {}", actual.style.as_str(), e.style)); }
        for (name, got, want) in [
            ("start", actual.start, e.start),
            ("fade", actual.fade, e.fade),
            ("cue", actual.incoming_cue, e.cue),
            ("incoming rate", actual.incoming_rate, e.incoming_rate),
            ("outgoing rate", actual.outgoing_rate, e.outgoing_rate),
        ] {
            if (got - want).abs() > 0.000_001 { diffs.push(format!("{name} {got} != {want}")); }
        }
        let policy: Vec<String> = actual.policy_reasons.iter().map(|s| s.to_string()).collect();
        if policy != e.policy_reasons { diffs.push(format!("policy {policy:?} != {:?}", e.policy_reasons)); }
        if !diffs.is_empty() {
            failures.push(format!("{} [{}]: {}", path.file_name().unwrap().to_string_lossy(), case.name, diffs.join("; ")));
        }
    }
    assert!(failures.is_empty(), "planner disagrees with Android:\n{}", failures.join("\n"));
}