/*
 * Ported from Orchard (https://github.com/SFG5453/Orchard).
 * Copyright (C) 2026 SFG545 (original Orchard implementation)
 * Copyright (C) 2026 Kushagra Singh (BitChord adaptation)
 *
 * Rust port of BitChord v1.9.1 shared/.../playback/smart/TransitionPlanner.kt.
 *
 * Orchard's original source is licensed under the GNU Affero General Public
 * License, version 3 or later. Per AGPLv3 section 13, this file is combined
 * here into BitChord -- a work licensed under the GNU General Public
 * License, version 3 or later -- and remains itself governed by the AGPLv3
 * as part of that combination.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
 * General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use super::policy::{self, Tier};
use super::TransitionSettings;
use std::time::Duration;

mod adaptive;
mod helpers;
mod phrase;

use adaptive::adaptive_plan;
use helpers::*;
use phrase::phrase_plan;

#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub id: String,
    pub status: String,
    pub duration: f64,
    pub bpm: f64,
    pub beat_confidence: f64,
    pub beat_interval: f64,
    pub downbeats: Vec<f64>,
    pub phrase_boundaries: Vec<f64>,
    pub first_beat: f64,
    pub key: String,
    pub key_confidence: f64,
    pub audible_start: f64,
    pub content_end: f64,
    pub energy: Vec<EnergySample>,
    pub low_energy: Vec<EnergySample>,
    pub vocal_probability: f64,
    pub mix_in_candidates: Vec<helpers::MixCandidate>,
}

impl Analysis {
    pub fn interval(&self) -> f64 {
        if self.beat_interval > 0.0 { self.beat_interval } else if self.bpm > 0.0 { 60.0 / self.bpm } else { 0.5 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: String,
    pub duration_ms: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: String,
    pub analysis: Option<Analysis>,
}

#[derive(Clone, Copy, Debug)]
pub struct EnergySample {
    pub time: f64,
    pub energy: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanStyle {
    #[default]
    EqualPower,
    Gapless,
    DjBlend,
    DjFilter,
}

impl PlanStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EqualPower => "EQUAL_POWER",
            Self::Gapless => "GAPLESS",
            Self::DjBlend => "DJ_BLEND",
            Self::DjFilter => "DJ_FILTER",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub blocked: bool,
    pub should_start: bool,
    pub reason: String,
    pub start: f64,
    pub end: f64,
    pub fade: f64,
    pub style: PlanStyle,
    pub incoming_cue: f64,
    pub incoming_handoff: f64,
    pub incoming_rate: f64,
    pub outgoing_rate: f64,
    pub transition_beats: u32,
    pub policy_reasons: Vec<&'static str>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            blocked: false, should_start: false, reason: String::new(),
            start: 0.0, end: 0.0, fade: 0.0, style: PlanStyle::EqualPower,
            incoming_cue: 0.0, incoming_handoff: 0.0,
            incoming_rate: 1.0, outgoing_rate: 1.0,
            transition_beats: 0, policy_reasons: Vec::new(),
        }
    }
}

/// Plans one transition, then confines it to the mix window when beat-aware is on.
pub fn plan_transition(outgoing: &Track, incoming: &Track, position: Duration, settings: &TransitionSettings) -> Plan {
    let plan = plan_inner(outgoing, incoming, position, settings);
    let length = outgoing.analysis.as_ref().map_or(0.0, |a| a.duration).max(outgoing.duration_ms as f64 / 1000.0);
    let window = settings.config.options_duration(Duration::from_secs_f64(length)).mix_window;
    if window { clamp_to_mix_window(plan, length, position.as_secs_f64()) } else { plan }
}

const MIX_WINDOW_SECONDS: f64 = 30.0;

/// A mix lives within the last 30 s of the outgoing track and the first 30 s of the incoming one.
fn clamp_to_mix_window(mut plan: Plan, length: f64, position: f64) -> Plan {
    if plan.blocked || length <= 0.0 {
        return plan;
    }
    let earliest = (length - MIX_WINDOW_SECONDS).max(0.0);
    if plan.start < earliest {
        let end = plan.end.max(earliest);
        plan.should_start = position >= earliest;
        plan.start = earliest;
        plan.end = end;
        plan.fade = (end - earliest).max(0.0);
    }
    if plan.incoming_cue > MIX_WINDOW_SECONDS {
        plan.incoming_cue = MIX_WINDOW_SECONDS;
    }
    plan
}

/// The decision ladder. Its default path is the untouched desktop Automix.
fn plan_inner(outgoing: &Track, incoming: &Track, position: Duration, settings: &TransitionSettings) -> Plan {
    let options = settings.config.options_duration(outgoing.analysis.as_ref().map_or(Duration::ZERO, |a| Duration::from_secs_f64(a.duration)));
    let position = position.as_secs_f64();
    let length = outgoing.analysis.as_ref().map_or(0.0, |a| a.duration).max(outgoing.duration_ms as f64 / 1000.0);
    if length <= 0.0 {
        return Plan::blocked("no-duration", 0.0);
    }
    let fade = settings.fallback_fade.as_secs_f64().clamp(1.0, 12.0);
    if !settings.automix {
        return standard(length, position, fade);
    }
    if length < 45.0 {
        return Plan::blocked("short-duration-guard", length);
    }
    let Some(out) = outgoing.analysis.as_ref() else {
        return standard(length, position, fade);
    };
    let Some(into) = incoming.analysis.as_ref() else {
        return standard(length, position, fade);
    };
    let content_end = if out.content_end > 0.0 { out.content_end.min(length) } else { length };
    if options.album_sequential && same_album(outgoing, incoming) {
        return Plan {
            start: (length - 0.45).max(0.0), end: length, fade: 0.12,
            style: PlanStyle::Gapless, ..Default::default()
        }.timed(position, "same-album-gapless", "before-gapless-window");
    }
    if spoken_or_live(outgoing) || spoken_or_live(incoming) {
        return Plan::blocked("blocked-speech-or-live", 0.0);
    }
    if !analysis_ready(out, outgoing) || !analysis_ready(into, incoming) {
        return standard_for(length, position, fade, "smart-analysis-fallback");
    }
    let policy = policy::assess(&tier_side(out), &tier_side(into), options.strict_confidence, options.allow_tempo_stretch);
    if policy.tier == Tier::PlainCrossfade {
        return Plan {
            start: (content_end - fade).max(0.0), end: content_end,
            fade: content_end - (content_end - fade).max(0.0),
            incoming_cue: incoming_start_point(into),
            incoming_handoff: incoming_start_point(into),
            policy_reasons: policy.reasons,
            ..Default::default()
        }.timed(position, "smart-plain-crossfade", "before-plain-crossfade-window");
    }
    let next_length = into.duration.max(incoming.duration_ms as f64 / 1000.0);
    if let Some(mut plan) = (options.phrase_bars == 8 || options.phrase_bars == 16)
        .then(|| phrase_plan(out, into, length, next_length, content_end, options.phrase_bars, &policy))
        .flatten()
        .filter(|plan| position < plan.end)
    {
        plan.policy_reasons = policy.reasons;
        let started = format!("smart-phrase-{}-bar", options.phrase_bars);
        let before = format!("before-phrase-{}-bar", options.phrase_bars);
        return cap_incoming_skip(plan, into, next_length).timed(position, &started, &before);
    }
    let mut plan = adaptive_plan(out, into, length, content_end, next_length, &policy);
    plan.policy_reasons = policy.reasons;
    if position < plan.end {
        let started = plan.reason.clone();
        let before = format!("before-{started}");
        cap_incoming_skip(plan, into, next_length).timed(position, &started, &before)
    } else {
        standard(length, position, fade)
    }
}

fn tier_side(value: &Analysis) -> policy::assess::Side {
    policy::assess::Side { bpm: value.bpm, confidence: value.beat_confidence }
}

fn analysis_ready(a: &Analysis, track: &Track) -> bool {
    a.status.is_empty() || a.status == "ready" && (a.id.is_empty() || track.id.is_empty() || a.id == track.id)
}

fn same_album(a: &Track, b: &Track) -> bool {
    !a.album_id.is_empty() && a.album_id == b.album_id
        || !a.album.is_empty() && a.album == b.album && a.artist == b.artist
}

fn spoken_or_live(track: &Track) -> bool {
    let text = format!("{} {} {}", track.title, track.artist, track.album).to_lowercase();
    ["podcast", "episode", "audiobook", "concert"].iter().any(|word| text.split_whitespace().any(|t| t == *word))
        || track.title.to_lowercase().contains("(live)")
        || track.title.to_lowercase().contains("[live")
        || track.title.to_lowercase().contains("- live")
        || track.album.to_lowercase().starts_with("live at")
        || track.album.to_lowercase().starts_with("live:")
}

fn standard(length: f64, position: f64, fade: f64) -> Plan {
    standard_for(length, position, fade, "standard")
}

fn standard_for(length: f64, position: f64, fade: f64, reason: &str) -> Plan {
    Plan { start: (length - fade).max(0.0), end: length, fade, ..Default::default() }
        .timed(position, reason, &format!("before-{reason}-window"))
}

impl Plan {
    fn blocked(reason: &str, at: f64) -> Self {
        Self { blocked: true, reason: reason.into(), start: at, end: at, ..Default::default() }
    }

    fn timed(mut self, position: f64, started: &str, before: &str) -> Self {
        self.should_start = position >= self.start;
        self.reason = if self.should_start { started.into() } else { before.into() };
        self
    }
}
