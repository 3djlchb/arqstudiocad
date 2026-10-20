//! Tests of laying texts out in strokes (a submodule of `text_layout`).

use std::f64::consts::FRAC_PI_2;

use super::*;

/// A left, baseline text at (0, 0), 9 units high (one font unit each).
fn frame(text: &str) -> TextFrame {
    TextFrame {
        lines: vec![text.to_owned()],
        first: (0.0, 0.0),
        second: None,
        height: CAP,
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        h: HAlign::Left,
        v: VAlign::Baseline,
        backward: false,
        upside_down: false,
        normal: PlanNormal::Up,
        wrap: None,
    }
}

fn bounds(f: &TextFrame) -> Bounds2 {
    layout(f).expect("laid out").bounds
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_font_reads_letters_accents_and_spacing() {
    let font = font();
    assert!(font.glyphs.len() > 200);
    assert!(close(font.glyphs[&'A'].width, 6.0));
    // An accented letter includes its base letter and adds the accent.
    let a = font.glyphs[&'a'].strokes.len();
    assert!(font.glyphs[&'á'].strokes.len() > a);
    assert!(font.glyphs.contains_key(&'ñ') && font.glyphs.contains_key(&'°'));
    assert_eq!((font.letter_spacing, font.word_spacing), (3.0, 6.75));
    let (_, ab) = line_strokes("AB");
    let (_, a_space_b) = line_strokes("A B");
    let width_b = font.glyphs[&'B'].width;
    assert!(close(ab, 6.0 + 3.0 + width_b));
    assert!(close(a_space_b, 6.0 + 3.0 + 6.75 + width_b));
    let (strokes, _) = line_strokes("\u{4e2d}"); // not in the font
    assert!(!strokes.is_empty(), "drawn as the replacement glyph");
}

#[test]
fn arcs_are_flattened_on_their_circle() {
    // 'O' is drawn with arcs: every point lies within its box, many of them.
    let (strokes, width) = line_strokes("O");
    assert!(strokes.len() > 8);
    for (a, b) in strokes {
        for (x, y) in [a, b] {
            assert!((-0.01..=width + 0.01).contains(&x) && (-0.01..=9.01).contains(&y));
        }
    }
}

#[test]
fn justification_places_the_text_around_its_anchor() {
    let width = line_strokes("AH").1;
    let left = bounds(&frame("AH"));
    assert!(close(left.min().x(), 0.0) && close(left.min().y(), 0.0));
    assert!(close(left.max().x(), width) && close(left.max().y(), 9.0));

    let at = |h, v| {
        let mut f = frame("AH");
        f.h = h;
        f.v = v;
        f.second = Some((100.0, 50.0));
        bounds(&f)
    };
    let right = at(HAlign::Right, VAlign::Baseline);
    assert!(close(right.max().x(), 100.0) && close(right.min().y(), 50.0));
    let center = at(HAlign::Center, VAlign::Top);
    assert!(close(center.min().x() + center.max().x(), 200.0));
    assert!(
        close(center.max().y(), 50.0),
        "Top: the cap line on the anchor"
    );
    let middle = at(HAlign::Middle, VAlign::Baseline);
    assert!(close(middle.min().y() + middle.max().y(), 100.0));
    let bottom = at(HAlign::Left, VAlign::Bottom);
    assert!(
        close(bottom.min().y(), 50.0 + DESCENDER),
        "Bottom: below the descenders"
    );
}

#[test]
fn rotation_scale_width_and_oblique() {
    let mut f = frame("AH");
    f.rotation = FRAC_PI_2;
    let up = bounds(&f);
    assert!(
        close(up.max().x(), 0.0) && close(up.min().x(), -9.0),
        "reads upwards"
    );
    let mut f = frame("AH");
    f.height = 18.0;
    f.width_factor = 0.5;
    let narrow = bounds(&f);
    assert!(close(narrow.max().y(), 18.0));
    assert!(close(narrow.max().x(), line_strokes("AH").1));
    let mut f = frame("H");
    f.oblique = std::f64::consts::FRAC_PI_4;
    let slanted = bounds(&f);
    let width = line_strokes("H").1;
    assert!(
        close(slanted.max().x(), width + 9.0),
        "the top leans 9 units right"
    );
}

#[test]
fn aligned_and_fit_fill_the_span_between_their_points() {
    let width = line_strokes("AH").1;
    let mut f = frame("AH");
    f.h = HAlign::Aligned;
    f.second = Some((2.0 * width, 0.0));
    let aligned = bounds(&f);
    assert!(close(aligned.max().x(), 2.0 * width) && close(aligned.max().y(), 18.0));
    f.h = HAlign::Fit;
    let fit = bounds(&f);
    assert!(close(fit.max().x(), 2.0 * width) && close(fit.max().y(), 9.0));
}

#[test]
fn mirrored_ocs_and_generation_flags_mirror_the_letters() {
    let mut f = frame("AH");
    f.normal = PlanNormal::Down;
    let down = bounds(&f);
    assert!(
        close(down.max().x(), 0.0) && down.min().x() < -5.0,
        "drawn to the west"
    );
    let mut f = frame("AH");
    f.backward = true;
    assert_eq!(bounds(&f), down);
    let mut f = frame("AH");
    f.upside_down = true;
    let flipped = bounds(&f);
    assert!(close(flipped.max().y(), 0.0) && close(flipped.min().y(), -9.0));
}

#[test]
fn multiline_text_stacks_its_lines() {
    let mut f = frame("A");
    f.lines = vec!["A".to_owned(), "A".to_owned()];
    f.v = VAlign::Top;
    let two = bounds(&f);
    assert!(
        close(two.max().y(), 0.0),
        "Top: the first cap line on the anchor"
    );
    assert!(close(two.min().y(), -9.0 - LINE_SPACING * 9.0));
}

#[test]
fn mtext_codes_leave_plain_lines() {
    assert_eq!(
        plain_mtext("{\\fArial|b0|i0;Hola}\\Pmundo \\S1^2;\\~\\H2.5x;fin"),
        ["Hola", "mundo 1/2 fin"]
    );
    assert_eq!(plain_mtext("a\\\\b \\{c\\} \\Lsub\\l"), ["a\\b {c} sub"]);
}

#[test]
fn texts_that_cannot_be_placed_are_refused() {
    for height in [0.0, -1.0, f64::NAN] {
        let mut f = frame("A");
        f.height = height;
        assert!(layout(&f).is_none());
    }
    let mut f = frame("   ");
    f.first = (4.0, 5.0);
    let empty = layout(&f).unwrap();
    assert!(empty.strokes.is_empty());
    assert!(close(empty.bounds.min().x(), 4.0) && close(empty.bounds.max().y(), 5.0));
}

#[test]
fn a_bulge_of_one_is_a_half_circle_turning_counterclockwise() {
    // From (0, 0) to (2, 0) counterclockwise: below the chord, round (1, 0).
    let mut segments = Vec::new();
    push_stroke(&[(0.0, 0.0, 0.0), (2.0, 0.0, 1.0)], 0.0, &mut segments);
    assert!(segments.len() >= 4);
    for (a, _) in &segments {
        assert!(
            close(((a.0 - 1.0).powi(2) + a.1.powi(2)).sqrt(), 1.0),
            "{a:?}"
        );
    }
    assert!(
        segments.iter().any(|(a, _)| a.1 < -0.9),
        "it passes by (1, -1)"
    );
    assert_eq!(segments.last().unwrap().1, (2.0, 0.0));
}

#[test]
fn a_smaller_bulge_bends_round_a_center_off_the_chord() {
    // Bulge 0.5 from (0, 0) to (2, 0): sagitta 0.5 below the chord, so the
    // center is (1, 0.75) and the radius 1.25.
    let mut segments = Vec::new();
    push_stroke(&[(0.0, 0.0, 0.0), (2.0, 0.0, 0.5)], 0.0, &mut segments);
    for (a, b) in &segments {
        for q in [a, b] {
            assert!(
                close(((q.0 - 1.0).powi(2) + (q.1 - 0.75).powi(2)).sqrt(), 1.25),
                "{q:?}"
            );
        }
    }
    let lowest = segments.iter().map(|(a, _)| a.1).fold(f64::MAX, f64::min);
    assert!(lowest < -0.45, "it bends down to (1, -0.5): {lowest}");
}

#[test]
fn paragraphs_wrap_at_spaces_to_the_width() {
    let word = line_strokes("AAA").1;
    let two = line_strokes("AAA AAA").1;
    assert_eq!(wrap_line("AAA AAA AAA", two), ["AAA AAA", "AAA"]);
    assert_eq!(wrap_line("AAA AAA", word), ["AAA", "AAA"]);
    assert_eq!(
        wrap_line("AAAAAAAAAA B", word),
        ["AAAAAAAAAA", "B"],
        "a long word stays whole"
    );
    assert_eq!(
        wrap_line("", word),
        [""],
        "an empty paragraph is an empty line"
    );

    let mut f = frame("AAA AAA AAA");
    f.v = VAlign::Top;
    let single = bounds(&f);
    f.wrap = Some(two);
    let wrapped = bounds(&f);
    assert!(
        close(single.min().y(), -9.0) && wrapped.min().y() < -9.0,
        "more lines"
    );
    assert!(wrapped.max().x() <= two + 1e-6);
}

#[test]
fn the_wrapping_width_is_in_drawing_units_whatever_the_height() {
    // Twice the height: the same words need twice the width per line.
    let two = line_strokes("AAA AAA").1;
    let mut f = frame("AAA AAA AAA");
    f.height = 2.0 * CAP;
    f.v = VAlign::Top;
    f.wrap = Some(2.0 * two);
    let wrapped = bounds(&f);
    assert!(
        close(wrapped.max().x(), 2.0 * two),
        "two words on the first line"
    );
    assert!(wrapped.min().y() < -2.0 * CAP, "the third word below");
}
