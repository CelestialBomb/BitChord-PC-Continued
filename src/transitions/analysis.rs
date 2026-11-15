//! Beat analysis for the desktop app, phase 1: tempo, grid confidence and a
//! first-beat estimate from the audio's own loudness changes. No new
//! dependencies, no downbeats. Phase 2 (design doc section 7) replaces this with
//! the Beat This! model through ONNX so phrase-aligned plans become possible.
//!
//! The existing `Tempo` meter in audio.rs only returns a tempo, with no sense of
//! how much to trust it. This one reports a 0..1 confidence in the same range as
//! Android's `beatConfidence`, so the shared policy can fall back to a plain
//! crossfade when the evidence is weak.

/// A reading of one stretch of audio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    pub confidence: f64,
    /// Seconds from the start of the stretch to the first strong onset.
    pub first_beat: f64,
}

/// Confidence ceiling, matching Android: a heuristic is evidence, not proof.
const MAX_CONFIDENCE: f64 = 0.95;
const MIN_LAG_SECONDS: f64 = 0.30;
const MAX_LAG_SECONDS: f64 = 1.20;
/// An autocorrelation peak weaker than this is treated as no beat at all.
const MIN_PEAK: f64 = 0.08;

/// Estimates tempo from onset strengths (`flux`, one per hop of `hop_seconds`).
///
/// Returns `None` when there is too little signal, or no repeating spacing.
pub fn estimate(flux: &[f32], hop_seconds: f64) -> Option<Grid> {
    let n = flux.len();
    if n < 64 || hop_seconds <= 0.0 {
        return None;
    }
    let mean = flux.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
    let power = flux.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / n as f64;
    if power <= 1e-12 {
        return None;
    }
    let lo = ((MIN_LAG_SECONDS / hop_seconds) as usize).max(4);
    let hi = ((MAX_LAG_SECONDS / hop_seconds) as usize).min(n / 2);
    if lo >= hi {
        return None;
    }
    let mut best = (0.0f64, 0usize);
    let mut scores = Vec::with_capacity(hi - lo + 1);
    for lag in lo..=hi {
        let sum: f64 = (lag..n).map(|i| (flux[i] as f64 - mean) * (flux[i - lag] as f64 - mean)).sum();
        let correlation = sum / (n - lag) as f64 / power;
        // The same mid-range prior the existing meter uses, so half- and
        // double-time patterns land on the tempo people would tap.
        let seconds = lag as f64 * hop_seconds;
        let prior = (-((seconds / 0.5).ln().powi(2)) / (2.0 * 0.4 * 0.4)).exp();
        let score = correlation * prior;
        scores.push(score);
        if score > best.0 {
            best = (score, lag);
        }
    }
    if best.0 < MIN_PEAK {
        return None;
    }
    let mut bpm = 60.0 / (best.1 as f64 * hop_seconds);
    while bpm < 70.0 {
        bpm *= 2.0;
    }
    while bpm > 180.0 {
        bpm /= 2.0;
    }
    let average = scores.iter().sum::<f64>() / scores.len() as f64;
    let confidence = grid_confidence(best.0, average);
    Some(Grid { bpm, confidence, first_beat: first_onset(flux, hop_seconds, mean, power) })
}

/// How far a tempo reading can be trusted, 0..1, from its best score and the
/// average score over every candidate spacing. Trust grows with how much the best
/// spacing stands out and with the peak's own strength. Also used by the playing
/// song's live meter in audio.rs, so both agree on what "confident" means.
pub fn grid_confidence(best: f64, average: f64) -> f64 {
    let prominence = if best > 0.0 { ((best - average) / best).clamp(0.0, 1.0) } else { 0.0 };
    let strength = (best / 0.5).clamp(0.0, 1.0);
    (0.15 + 0.45 * prominence + 0.35 * strength).clamp(0.0, MAX_CONFIDENCE)
}

fn first_onset(flux: &[f32], hop_seconds: f64, mean: f64, power: f64) -> f64 {
    let threshold = mean + power.sqrt();
    flux.iter().position(|v| *v as f64 >= threshold).map_or(0.0, |i| i as f64 * hop_seconds)
}

/// Onset strength: the positive change in loudness between frames.
pub fn onset_strength(levels: &[f32]) -> Vec<f32> {
    let mut previous = 0.0f32;
    levels
        .iter()
        .map(|level| {
            let rise = (*level - previous).max(0.0);
            previous = *level;
            rise
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click track: one onset every `period` hops, padded with silence.
    fn clicks(period: usize, hops: usize) -> Vec<f32> {
        (0..hops).map(|i| if i % period == 0 { 1.0 } else { 0.0 }).collect()
    }

    #[test]
    fn steady_clicks_give_the_tempo_and_a_trusted_grid() {
        // 46.4 ms hops, a click every 11 hops is 0.51 s, close to 117 BPM.
        let hop = 0.0464;
        let grid = estimate(&clicks(11, 600), hop).expect("a steady pulse has a tempo");
        let expected = 60.0 / (11.0 * hop);
        assert!((grid.bpm - expected).abs() < 3.0, "bpm {} vs {expected}", grid.bpm);
        assert!(grid.confidence >= 0.55, "a clean grid should be trusted, got {}", grid.confidence);
        assert!(grid.confidence <= MAX_CONFIDENCE);
    }

    #[test]
    fn silence_and_noise_without_pulse_have_no_grid() {
        assert_eq!(estimate(&vec![0.0; 600], 0.0464), None);
        assert_eq!(estimate(&[], 0.0464), None);
        assert_eq!(estimate(&[1.0; 10], 0.0464), None);
    }

    #[test]
    fn onset_strength_only_counts_rises() {
        assert_eq!(onset_strength(&[0.0, 1.0, 0.5, 0.75]), vec![0.0, 1.0, 0.0, 0.25]);
    }

    #[test]
    fn irregular_clicks_are_less_trusted_than_a_steady_pulse() {
        let steady = estimate(&clicks(11, 600), 0.0464).unwrap().confidence;
        let mut ragged = vec![0.0f32; 600];
        let mut at = 0usize;
        let mut step = 7usize;
        while at < 600 {
            ragged[at] = 1.0;
            step = 7 + (step * 5 + 3) % 9;
            at += step;
        }
        // No grid at all is also an acceptable verdict on a ragged pulse.
        if let Some(grid) = estimate(&ragged, 0.0464) {
            assert!(grid.confidence < steady, "{} !< {steady}", grid.confidence);
        }
    }
}
