use super::*;

pub(super) fn incoming_start_point(a: &Analysis) -> f64 {
    [a.audible_start, a.first_beat].into_iter().find(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0)
}

fn nearest(values: &[f64], target: f64, tolerance: f64, minimum: f64) -> Option<f64> {
    values.iter().copied()
        .filter(|v| v.is_finite() && *v >= minimum && (*v - target).abs() <= tolerance)
        .min_by(|a, b| (*a - target).abs().total_cmp(&(*b - target).abs()))
}

fn at_or_before(values: &[f64], target: f64, tolerance: f64, minimum: f64) -> Option<f64> {
    values.iter().copied()
        .filter(|v| v.is_finite() && *v >= minimum && *v <= target && target - *v <= tolerance)
        .max_by(|a, b| a.total_cmp(b))
}

pub(super) fn aligned_start(a: &Analysis, target: f64, end: f64, prefer_earlier: bool, minimum: f64) -> f64 {
    let interval = a.interval();
    let phrase_tolerance = (interval * 4.0).max(1.0);
    let downbeat_tolerance = (interval * 2.0).max(0.75);
    let pick = |values: &[f64], tolerance: f64| {
        if prefer_earlier { at_or_before(values, target, tolerance, minimum) } else { nearest(values, target, tolerance, minimum) }
    };
    policy::clamp(
        pick(&a.phrase_boundaries, phrase_tolerance)
            .or_else(|| pick(&a.downbeats, downbeat_tolerance)).unwrap_or(target),
        minimum,
        end,
    )
}

pub(super) fn downbeat_before(a: &Analysis, time: f64, tolerance: f64) -> f64 {
    at_or_before(&a.downbeats, time, tolerance, 0.0).unwrap_or(time)
}

pub(super) fn incoming_cue_point(a: &Analysis, latest: f64) -> f64 {
    if let Some(candidate) = a.mix_in_candidates.iter()
        .filter(|c| c.time <= latest)
        .max_by(|a, b| a.score.total_cmp(&b.score))
    {
        return candidate.time;
    }
    nearest(&a.downbeats, first_downbeat(a), a.interval().max(0.5), 0.0)
        .unwrap_or_else(|| first_downbeat(a))
        .min(latest)
}

fn first_downbeat(a: &Analysis) -> f64 {
    a.downbeats.get(4).copied().or_else(|| a.downbeats.first().copied()).unwrap_or(incoming_start_point(a))
}

pub(super) fn cap_incoming_skip(mut plan: Plan, a: &Analysis, next_length: f64) -> Plan {
    let latest = next_length * 0.25;
    if plan.incoming_cue <= latest {
        return plan;
    }
    let shift = if a.interval() > 0.0 {
        ((plan.incoming_cue - latest) / (4.0 * a.interval())).ceil() * 4.0 * a.interval()
    } else {
        plan.incoming_cue - latest
    };
    let cue = (plan.incoming_cue - shift).max(0.0);
    plan.incoming_handoff = (plan.incoming_handoff - (plan.incoming_cue - cue)).max(cue);
    plan.incoming_cue = cue;
    plan
}

pub(super) fn average_energy(curve: &[EnergySample], from: f64, until: f64) -> Option<f64> {
    if until <= from {
        return None;
    }
    let mut sum = 0.0;
    let mut count = 0u64;
    for point in curve.iter().skip_while(|point| point.time < from) {
        if point.time >= until {
            break;
        }
        if point.energy.is_finite() && point.energy >= 0.0 {
            sum += point.energy;
            count += 1;
        }
    }
    (count > 0).then_some(sum / count as f64)
}

pub(super) fn mean_energy(curve: &[EnergySample]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0u64;
    for point in curve.iter().filter(|point| point.energy.is_finite() && point.energy >= 0.0) {
        sum += point.energy;
        count += 1;
    }
    if count > 0 { sum / count as f64 } else { 0.0 }
}

pub(super) fn normalized_tempo_ratio(current: f64, next: f64) -> f64 {
    if current <= 0.0 || next <= 0.0 {
        return 1.0;
    }
    let mut ratio = next / current;
    while ratio > 1.5 {
        ratio /= 2.0;
    }
    while ratio < 0.67 {
        ratio *= 2.0;
    }
    ratio
}

pub(super) fn whole_bars(seconds: f64, beat_seconds: f64, maximum: f64) -> f64 {
    if beat_seconds <= 0.0 {
        return seconds;
    }
    let bar = 4.0 * beat_seconds;
    let mut bars = (seconds / bar).round();
    if bars * bar > maximum + 1e-6 {
        bars = (maximum / bar + 1e-6).floor();
    }
    if bars >= 1.0 { bars * bar } else { seconds }
}

#[derive(Clone, Debug)]
pub struct MixCandidate {
    pub time: f64,
    pub score: f64,
    pub kind: String,
}
