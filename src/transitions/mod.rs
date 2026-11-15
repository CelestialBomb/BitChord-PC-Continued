//! Transition configuration shared by the audio engine, UI and tests.
//!
//! PC phase 1 does not send decoded audio off the machine. Everything here is
//! a small value type so the runner can use exactly the same rules as playback.
//! Spec: docs/transitions-design.md (BitChord PC Continued v2.0.1).

// Phase 1 status (design doc section 7): the engine uses `policy` (the shared confidence
// floor) and `analysis::grid_confidence` today. The planner below is the Rust port of
// Android's rules and is exercised by the shared fixtures, but it does not yet choose
// where a blend starts, so most of it is not called from playback. Remove this allowance
// when the engine is driven by `plan_transition`.
#![allow(dead_code)]

pub mod analysis;
#[cfg(test)]
mod fixture;
pub mod plan;
pub mod policy;

use plan::Plan;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatAwareConfig {
    pub enabled: bool,
    pub phrase_aligned: bool,
    pub tempo_matching: bool,
    pub album_gapless: bool,
}

impl Default for BeatAwareConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            phrase_aligned: true,
            tempo_matching: true,
            album_gapless: true,
        }
    }
}

impl BeatAwareConfig {
    /// Same mapping as Android's BeatAwareConfig.optionsFor. Disabled is a no-op.
    pub fn options_duration(&self, outgoing: Duration) -> PlannerOptions {
        if !self.enabled {
            return PlannerOptions::default();
        }
        PlannerOptions {
            phrase_bars: if self.phrase_aligned {
                if outgoing >= Duration::from_secs(240) { 16 } else { 8 }
            } else {
                0
            },
            album_sequential: self.album_gapless,
            mix_window: true,
            strict_confidence: true,
            allow_tempo_stretch: self.tempo_matching,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannerOptions {
    pub phrase_bars: u32,
    pub album_sequential: bool,
    pub mix_window: bool,
    pub strict_confidence: bool,
    pub allow_tempo_stretch: bool,
}

impl Default for PlannerOptions {
    fn default() -> Self {
        Self {
            phrase_bars: 0,
            album_sequential: false,
            mix_window: false,
            strict_confidence: false,
            allow_tempo_stretch: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransitionSettings {
    pub automix: bool,
    pub config: BeatAwareConfig,
    pub fallback_fade: Duration,
}

impl Default for TransitionSettings {
    fn default() -> Self {
        Self { automix: true, config: BeatAwareConfig::default(), fallback_fade: Duration::from_secs(8) }
    }
}

pub fn plan_transition(
    outgoing: &plan::Track,
    incoming: &plan::Track,
    position: Duration,
    settings: &TransitionSettings,
) -> Plan {
    plan::plan_transition(outgoing, incoming, position, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_config_is_the_original_android_mapping() {
        let off = BeatAwareConfig { enabled: false, phrase_aligned: false, tempo_matching: false, album_gapless: false };
        assert_eq!(off.options_duration(Duration::from_secs(300)), PlannerOptions::default());
    }

    #[test]
    fn phrase_length_matches_android_eight_or_sixteen() {
        let on = BeatAwareConfig { enabled: true, ..Default::default() };
        assert_eq!(on.options_duration(Duration::from_secs(199)).phrase_bars, 8);
        assert_eq!(on.options_duration(Duration::from_secs(240)).phrase_bars, 16);
        let plain = BeatAwareConfig { enabled: true, phrase_aligned: false, tempo_matching: false, album_gapless: false };
        let options = plain.options_duration(Duration::from_secs(300));
        assert_eq!(options.phrase_bars, 0);
        assert!(!options.album_sequential);
        assert!(!options.allow_tempo_stretch);
        // The two safety rules stay on for the whole feature.
        assert!(options.strict_confidence);
        assert!(options.mix_window);
    }
}
