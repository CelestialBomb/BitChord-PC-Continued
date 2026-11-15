/*
 * Ported from Orchard (https://github.com/SFG5453/Orchard).
 *
 * Copyright (C) 2026 SFG545 (original Orchard implementation)
 * Copyright (C) 2026 Kushagra Singh (BitChord adaptation)
 *
 * Rust port follows `shared/src/commonMain/kotlin/com/music/bitchord/playback/
 * smart/TransitionPolicy.kt` in BitChord v1.9.1, with the opt-in beat-aware
 * flags from our `transitions` branch. The policy and its tests, not PCM,
 * decide how ambitious a transition the stored evidence supports.
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

pub const MIN_BEATMATCH_CONFIDENCE: f64 = 0.55;
pub const MIN_DJ_CONFIDENCE: f64 = 0.2;
pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 220.0;
pub const MIN_INCOMING_RATE: f64 = 0.9;
pub const MAX_INCOMING_RATE: f64 = 1.25;
pub const MAX_DISCARDED_MUSIC_SECONDS: f64 = 12.0;
pub const DISCIPLINE_TOLERANCE: f64 = 1e-9;
pub const STRETCH_DISABLED: &str = "stretch-disabled";

pub fn or_zero(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value.is_finite() { min.max(max.min(value)) } else { min }
}

pub fn align_tempo_octave(outgoing: f64, incoming: f64) -> f64 {
    if outgoing <= 0.0 || incoming <= 0.0 {
        return incoming;
    }
    let mut aligned = incoming;
    while aligned / outgoing > 1.5 {
        aligned /= 2.0;
    }
    while aligned / outgoing < 0.67 {
        aligned *= 2.0;
    }
    aligned
}

pub fn incoming_rate_between(outgoing: f64, incoming: f64) -> f64 {
    if outgoing <= 0.0 || incoming <= 0.0 {
        return f64::NAN;
    }
    outgoing / align_tempo_octave(outgoing, incoming)
}

pub fn requires_tempo_stretch(outgoing: f64, incoming: f64) -> bool {
    let rate = incoming_rate_between(outgoing, incoming);
    rate.is_finite() && (rate - 1.0).abs() > DISCIPLINE_TOLERANCE
}

pub fn stretch_reachable(outgoing: f64, incoming: f64) -> bool {
    let rate = incoming_rate_between(outgoing, incoming);
    rate.is_finite()
        && (MIN_INCOMING_RATE - DISCIPLINE_TOLERANCE..=MAX_INCOMING_RATE + DISCIPLINE_TOLERANCE).contains(&rate)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Beatmatched,
    DjAssisted,
    PlainCrossfade,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    pub tier: Tier,
    pub reasons: Vec<&'static str>,
    pub confidence: f64,
}

pub fn assess(outgoing: &assess::Side, incoming: &assess::Side, strict: bool, allow_stretch: bool) -> Policy {
    let outgoing_bpm = or_zero(outgoing.bpm);
    let incoming_bpm = or_zero(incoming.bpm);
    let outgoing_confidence = or_zero(outgoing.confidence);
    let incoming_confidence = or_zero(incoming.confidence);
    let confidence = outgoing_confidence.min(incoming_confidence);
    let mut reasons = Vec::new();
    if !((MIN_BPM..=MAX_BPM).contains(&outgoing_bpm)) {
        reasons.push("outgoing-tempo");
    }
    if !((MIN_BPM..=MAX_BPM).contains(&incoming_bpm)) {
        reasons.push("incoming-tempo");
    }
    if !reasons.is_empty() {
        return Policy { tier: Tier::PlainCrossfade, reasons, confidence };
    }
    if outgoing_confidence < MIN_DJ_CONFIDENCE && incoming_confidence < MIN_DJ_CONFIDENCE
        || strict && (outgoing_confidence < MIN_DJ_CONFIDENCE || incoming_confidence < MIN_DJ_CONFIDENCE)
    {
        return Policy { tier: Tier::PlainCrossfade, reasons: vec!["beat-confidence"], confidence };
    }
    let reachable = stretch_reachable(outgoing_bpm, incoming_bpm);
    if !allow_stretch && reachable && requires_tempo_stretch(outgoing_bpm, incoming_bpm) {
        reasons.push(STRETCH_DISABLED);
    } else if !reachable {
        reasons.push("tempo-distance");
    }
    if outgoing_confidence < MIN_BEATMATCH_CONFIDENCE || incoming_confidence < MIN_BEATMATCH_CONFIDENCE {
        reasons.push("beat-confidence");
    }
    Policy {
        tier: if reasons.is_empty() { Tier::Beatmatched } else { Tier::DjAssisted },
        reasons,
        confidence,
    }
}

/// The tempo and grid confidence the analysis was able to measure for one track.
pub mod assess {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Side {
        pub bpm: f64,
        pub confidence: f64,
    }
}
