//! World-space linetype pattern traversal shared by the CPU and GPU paths.

/// One visible element of a linetype pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PatternPart {
    /// A positive dash interval.
    Dash((f64, f64), (f64, f64)),
    /// A zero-length dot interval.
    Dot((f64, f64)),
}

/// Emits the visible dash pieces and dot locations of a segment. `phase` is
/// the distance already traveled through this entity's repeating pattern.
/// The pattern lengths are multiplied by `scale`; negative values are gaps,
/// positive values are dashes and zero values are dots.
pub(crate) fn split_segment(
    a: (f64, f64),
    b: (f64, f64),
    pattern: &[f64],
    scale: f64,
    phase: &mut f64,
    clipped: Option<((f64, f64), (f64, f64))>,
    mut emit: impl FnMut(PatternPart),
) {
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    if pattern.is_empty()
        || !scale.is_finite()
        || scale <= 0.0
        || !length.is_finite()
        || length <= 0.0
    {
        if let Some((start, end)) = clipped {
            emit(PatternPart::Dash(start, end));
        }
        return;
    }
    let period: f64 = pattern.iter().map(|value| value.abs() * scale).sum();
    if !period.is_finite() || period <= 0.0 {
        if let Some((start, end)) = clipped {
            emit(PatternPart::Dash(start, end));
        }
        return;
    }

    if let Some((start, end)) = clipped {
        let distance_to = |p: (f64, f64)| (p.0 - a.0).hypot(p.1 - a.1);
        let visible_start = distance_to(start).min(length);
        let visible_end = distance_to(end).min(length);
        let mut distance = visible_start;
        let stop = visible_end.max(visible_start);
        let mut pattern_position = (*phase + distance).rem_euclid(period);
        let mut index = 0;
        for _ in 0..pattern.len() {
            let size = pattern[index].abs() * scale;
            if size > 0.0 && pattern_position >= size {
                pattern_position -= size;
                index = (index + 1) % pattern.len();
            } else {
                break;
            }
        }
        let mut within = pattern_position;
        let mut iterations = 0;
        while distance < stop && iterations < 100_000 {
            let value = pattern[index];
            let size = value.abs() * scale;
            if size == 0.0 {
                if value == 0.0 {
                    let point = interpolate(a, b, distance / length);
                    emit(PatternPart::Dot(point));
                }
                index = (index + 1) % pattern.len();
                within = 0.0;
                iterations += 1;
                continue;
            }
            let take = (size - within).min(stop - distance);
            if value > 0.0 && take > 0.0 {
                emit(PatternPart::Dash(
                    interpolate(a, b, distance / length),
                    interpolate(a, b, (distance + take) / length),
                ));
            }
            distance += take;
            within += take;
            if within >= size - f64::EPSILON {
                index = (index + 1) % pattern.len();
                within = 0.0;
            }
            iterations += 1;
        }
    }
    *phase = (*phase + length).rem_euclid(period);
}

fn interpolate(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_dashes_gaps_and_dots_and_carries_phase_between_segments() {
        let mut parts = Vec::new();
        let mut phase = 0.0;
        split_segment(
            (0.0, 0.0),
            (5.0, 0.0),
            &[2.0, -2.0],
            1.0,
            &mut phase,
            Some(((0.0, 0.0), (5.0, 0.0))),
            |part| parts.push(part),
        );
        assert_eq!(
            parts,
            [
                PatternPart::Dash((0.0, 0.0), (2.0, 0.0)),
                PatternPart::Dash((4.0, 0.0), (5.0, 0.0))
            ]
        );

        let mut next = Vec::new();
        split_segment(
            (5.0, 0.0),
            (7.0, 0.0),
            &[2.0, -2.0],
            1.0,
            &mut phase,
            Some(((5.0, 0.0), (7.0, 0.0))),
            |part| next.push(part),
        );
        assert_eq!(next, [PatternPart::Dash((5.0, 0.0), (6.0, 0.0))]);
    }

    #[test]
    fn dots_are_emitted_at_pattern_boundaries() {
        let mut dots = Vec::new();
        let mut phase = 0.0;
        split_segment(
            (0.0, 0.0),
            (5.0, 0.0),
            &[0.0, -2.0],
            1.0,
            &mut phase,
            Some(((0.0, 0.0), (5.0, 0.0))),
            |part| dots.push(part),
        );
        assert_eq!(
            dots,
            [
                PatternPart::Dot((0.0, 0.0)),
                PatternPart::Dot((2.0, 0.0)),
                PatternPart::Dot((4.0, 0.0))
            ]
        );
    }
}
