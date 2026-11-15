use super::*;

/// Adaptive Automix's overlap and cue calculation, not a second transition engine.
pub(super) fn adaptive_plan(
    out: &Analysis,
    into: &Analysis,
    _length: f64,
    anchor: f64,
    next_length: f64,
    verdict: &policy::Policy,
) -> Plan {
    let current = out.bpm;
    let next = into.bpm;
    let tempo_far = !policy::stretch_reachable(current, next)
        || verdict.reasons.contains(&policy::STRETCH_DISABLED);
    let grid_beat = if current > 0.0 && out.beat_confidence >= policy::MIN_DJ_CONFIDENCE { 60.0 / current } else { 0.0 };
    let energy_scale = energy_scale(out, into, anchor, grid_beat);
    let sweep = fallback_is_sweep(out, into, anchor, current, grid_beat);
    let rate = if tempo_far {
        1.0
    } else {
        (1.0 / normalized_tempo_ratio(current, next) * 10_000.0).round() / 10_000.0
    };
    let same_beat = rate >= 1.0 && current > 0.0 && next > 0.0
        && ((1.0 - normalized_tempo_ratio(current, next)).abs() <= 0.05
        && (out.beat_confidence >= 0.2 || into.beat_confidence >= 0.2)
        || out.beat_confidence >= policy::MIN_BEATMATCH_CONFIDENCE && into.beat_confidence >= policy::MIN_BEATMATCH_CONFIDENCE && !tempo_far);
    let overlap_beats = if tempo_far {
        if sweep { 12.0 } else { 8.0 }
    } else if key_clash(out, into) || vocal_conflict(out, into) {
        8.0
    } else {
        16.0
    };
    let beat_seconds = if current > 0.0 { 60.0 / current } else { 0.5 };
    let max_seconds = if same_beat {
        if energy_scale > 1.0 { 16.0 } else { 12.0 }
    } else if sweep {
        8.0
    } else {
        7.0
    };
    let minimum = if current >= 140.0 { 6.0 } else { 4.0 };
    let overlap = whole_bars((overlap_beats * energy_scale * beat_seconds).clamp(minimum, 16.0), beat_seconds, max_seconds);
    let arranged = if same_beat { (8.0 * beat_seconds).min(policy::MAX_DISCARDED_MUSIC_SECONDS) } else { 0.0 };
    let mix_end = if grid_beat > 0.0 { downbeat_before(out, anchor - arranged, 4.0 * grid_beat) } else { anchor };
    let max_beats_seconds = 32.0 * 60.0 / if current > 0.0 { current } else if next > 0.0 { next } else { 120.0 };
    let maximum_overlap = max_beats_seconds.min(max_seconds).min(mix_end * 0.4)
        .min(if next_length > 0.0 { next_length * 0.4 } else { max_seconds });
    let handoff_seconds = if current > 0.0 {
        (((if same_beat { 8.0 } else { 4.0 }) * 60.0) / current).clamp(2.0, if same_beat { 6.0 } else { 5.0 })
    } else {
        4.0
    };
    let cue = incoming_cue_point(into, next_length * 0.25);
    let requested_handoff = if same_beat {
        cue + 8.0 * 60.0 / policy::align_tempo_octave(current, next)
    } else {
        cue
    };
    let max_handoff = next_length - 5.0;
    let handoff = if max_handoff >= cue { requested_handoff.min(max_handoff) } else { cue };
    let (start, final_cue);
    if same_beat && beat_seconds > 0.0 {
        let total = whole_bars((handoff / rate.max(0.8)).clamp(maximum_overlap.min(12.0), maximum_overlap), beat_seconds, maximum_overlap);
        let target = (mix_end - total).max(0.0);
        start = aligned_start(out, target, mix_end - 0.05, true, (mix_end - maximum_overlap).max(0.0));
        final_cue = (handoff - (mix_end - start) * rate).max(0.0);
    } else {
        let actual = whole_bars(overlap.clamp(handoff_seconds.min(maximum_overlap), maximum_overlap), beat_seconds, maximum_overlap);
        let target = (mix_end - actual).max(0.0);
        // No shared grid to land on, so the nearest phrase or downbeat wins, as on Android.
        start = aligned_start(out, target, mix_end - 0.05, false, (mix_end - maximum_overlap).max(0.0));
        final_cue = incoming_start_point(into);
    }
    Plan {
        start, end: mix_end, fade: mix_end - start,
        style: if same_beat { PlanStyle::DjBlend } else { PlanStyle::DjFilter },
        incoming_cue: final_cue,
        incoming_handoff: handoff,
        incoming_rate: rate.clamp(policy::MIN_INCOMING_RATE, policy::MAX_INCOMING_RATE),
        transition_beats: overlap_beats as u32,
        reason: "smart-duration".into(),
        ..Default::default()
    }
}

fn energy_scale(out: &Analysis, into: &Analysis, anchor: f64, beat: f64) -> f64 {
    if beat <= 0.0 { return 1.0; }
    let window = 8.0 * beat;
    let (out_mean, in_mean) = (mean_energy(&out.energy), mean_energy(&into.energy));
    if out_mean <= 0.0 || in_mean <= 0.0 { return 1.0; }
    let exit = average_energy(&out.energy, anchor - window, anchor);
    let entry = average_energy(&into.energy, incoming_start_point(into), incoming_start_point(into) + window);
    match (exit, entry) {
        (Some(a), Some(b)) if a / out_mean < 0.7 && b / in_mean < 0.7 => 2.0,
        _ => 1.0,
    }
}

fn key_clash(out: &Analysis, into: &Analysis) -> bool {
    !out.key.is_empty() && !into.key.is_empty()
        && out.key_confidence >= 0.25 && into.key_confidence >= 0.25 && out.key != into.key
}

fn vocal_conflict(out: &Analysis, into: &Analysis) -> bool {
    out.vocal_probability >= 0.62 && into.vocal_probability >= 0.62
}

fn reference(samples: &[EnergySample]) -> Option<f64> {
    let mut energies: Vec<f64> = samples.iter().map(|p| p.energy)
        .filter(|value| value.is_finite() && *value >= 0.0).collect();
    energies.sort_by(|a, b| a.total_cmp(b));
    if energies.is_empty() { return None; }
    let upper_decile = energies[(energies.len() - 1) * 9 / 10];
    let reference = upper_decile.max(energies[energies.len() - 1] * 0.25);
    (reference > 1e-9).then_some(reference)
}

fn fallback_is_sweep(out: &Analysis, into: &Analysis, anchor: f64, current: f64, beat: f64) -> bool {
    if current <= 0.0 { return true; }
    let window = 8.0 * if beat > 0.0 { beat } else { 0.5 };
    let (Some(ref_out), Some(ref_in)) = (reference(&out.low_energy), reference(&into.low_energy)) else { return true };
    let outgoing = average_energy(&out.low_energy, anchor - window, anchor);
    let entry = incoming_start_point(into);
    let incoming = average_energy(&into.low_energy, entry, entry + window);
    match (outgoing, incoming) {
        (Some(a), Some(b)) => !(a / ref_out >= 0.5 && b / ref_in >= 0.5),
        _ => true,
    }
}
