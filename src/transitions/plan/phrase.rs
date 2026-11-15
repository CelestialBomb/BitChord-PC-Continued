use super::*;

/// The exact fixed-length phrase plan used by the Android Automix planner.
pub(super) fn phrase_plan(
    out: &Analysis,
    into: &Analysis,
    length: f64,
    next_length: f64,
    mix_anchor: f64,
    bars: u32,
    verdict: &policy::Policy,
) -> Option<Plan> {
    if !(policy::MIN_BPM..=policy::MAX_BPM).contains(&out.bpm) || out.interval() <= 0.0 {
        return None;
    }
    let beat_seconds = out.interval();
    let requested_overlap = bars as f64 * 4.0 * beat_seconds;
    if requested_overlap <= 0.0 || requested_overlap > 64.0 {
        return None;
    }
    let anchor = mix_anchor.min(length);
    let trusted = out.beat_confidence >= policy::MIN_DJ_CONFIDENCE;
    let mix_end = if trusted { downbeat_before(out, anchor, 4.0 * beat_seconds) } else { anchor };
    if mix_end <= 0.0 {
        return None;
    }
    let latest_start = mix_end - requested_overlap;
    if latest_start < 5.0 {
        return None;
    }
    let start = if trusted { aligned_start(out, latest_start, mix_end - 0.05, true, 5.0) } else { latest_start };
    let end = start + requested_overlap;
    if end > length + 0.05 {
        return None;
    }
    let tempo_far = into.bpm > 0.0 && (!policy::stretch_reachable(out.bpm, into.bpm)
        || !verdict.tier.eq(&Tier::Beatmatched) && !out.bpm.eq(&into.bpm) && verdict.reasons.contains(&policy::STRETCH_DISABLED));
    let media_ratio = if into.bpm > 0.0 && !tempo_far {
        (1.0 / normalized_tempo_ratio(out.bpm, into.bpm) * 10_000.0).round() / 10_000.0
    } else {
        1.0
    };
    let rate = media_ratio.clamp(policy::MIN_INCOMING_RATE, policy::MAX_INCOMING_RATE);
    let cue = incoming_start_point(into);
    let handoff = incoming_cue_point(into, next_length * 0.25).min(next_length - 5.0).max(cue);
    if next_length > 0.0 && cue + requested_overlap * rate + 5.0 > next_length {
        return None;
    }
    Some(Plan {
        start, end, fade: end - start,
        style: if verdict.tier == Tier::Beatmatched { PlanStyle::DjBlend } else { PlanStyle::DjFilter },
        incoming_cue: cue,
        incoming_handoff: handoff,
        incoming_rate: rate,
        outgoing_rate: 1.0,
        transition_beats: bars * 4,
        ..Default::default()
    })
}
