//! Lays texts out as strokes (ADR 0022 D1, amended): the letters of TEXT,
//! MTEXT and attributes, in world coordinates, with one built-in
//! single-stroke font (LibreCAD's ISO 3098 `standard.lff`, GPL-2.0-or-later,
//! see `THIRD_PARTY.md`). The drawing's own fonts (SHX, TrueType), MTEXT
//! formatting and word wrapping are phase 04: every text is drawn in this
//! font, and MTEXT shows its plain lines.
//!
//! No codec type appears here: `translate.rs` reads the entity into a
//! [`TextFrame`].

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::OnceLock;

use arq_geometry::{Bounds2, Point2};

use crate::ocs::PlanNormal;

/// The font's cap height, in its own units.
const CAP: f64 = 9.0;
/// Its descender depth (letters such as `g` reach −3).
const DESCENDER: f64 = 3.0;
/// Line spacing of multiline text, in text heights (AutoCAD's 5/3).
const LINE_SPACING: f64 = 5.0 / 3.0;

/// Horizontal justification, as DXF names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HAlign {
    Left,
    Center,
    Right,
    Aligned,
    Middle,
    Fit,
}

/// Vertical justification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VAlign {
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// What it takes to lay a text out, in its OCS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextFrame {
    /// The lines to draw (one for TEXT), already without formatting codes.
    pub(crate) lines: Vec<String>,
    /// First alignment point (the insertion point).
    pub(crate) first: (f64, f64),
    /// Second alignment point, when the justification uses it.
    pub(crate) second: Option<(f64, f64)>,
    pub(crate) height: f64,
    /// Radians, in the OCS.
    pub(crate) rotation: f64,
    pub(crate) width_factor: f64,
    pub(crate) oblique: f64,
    pub(crate) h: HAlign,
    pub(crate) v: VAlign,
    /// DXF generation flags: mirrored in X (backward), in Y (upside down).
    pub(crate) backward: bool,
    pub(crate) upside_down: bool,
    pub(crate) normal: PlanNormal,
    /// MTEXT: the width its paragraphs wrap to, in drawing units.
    pub(crate) wrap: Option<f64>,
}

/// A text laid out: its strokes and their box, in world coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub(crate) strokes: Vec<(Point2, Point2)>,
    pub(crate) bounds: Bounds2,
}

/// One glyph: polylines of `(x, y)` points in font units, each point with
/// the bulge of the arc that reaches it from the previous one (0: straight).
#[derive(Clone, Debug, Default)]
struct Glyph {
    strokes: Vec<Vec<(f64, f64, f64)>>,
    width: f64,
}

struct Font {
    glyphs: HashMap<char, Glyph>,
    letter_spacing: f64,
    word_spacing: f64,
}

fn font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| parse(include_str!("../fonts/standard.lff")))
}

/// Reads LibreCAD's LFF format: `[hhhh] c` headers, lines of `x,y` points
/// separated by `;` (`x,y,A<bulge>` ends an arc), and `Chhhh` lines that
/// include another glyph.
fn parse(source: &str) -> Font {
    let mut raw: Vec<(char, Vec<String>)> = Vec::new();
    let (mut letter_spacing, mut word_spacing) = (3.0, 6.75);
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('#') {
            let (key, value) = rest.split_once(':').unwrap_or((rest, ""));
            let value: Option<f64> = value.trim().parse().ok();
            match (key.trim(), value) {
                ("LetterSpacing", Some(v)) => letter_spacing = v,
                ("WordSpacing", Some(v)) => word_spacing = v,
                _ => {}
            }
        } else if let Some(header) = line.strip_prefix('[') {
            let code = header.split(']').next().unwrap_or("");
            if let Some(c) = u32::from_str_radix(code, 16).ok().and_then(char::from_u32) {
                raw.push((c, Vec::new()));
            }
        } else if !line.is_empty()
            && let Some((_, body)) = raw.last_mut()
        {
            body.push(line.to_owned());
        }
    }
    let bodies: HashMap<char, Vec<String>> = raw.into_iter().collect();
    let mut glyphs = HashMap::with_capacity(bodies.len());
    for &c in bodies.keys() {
        let mut glyph = Glyph::default();
        collect(c, &bodies, &mut glyph.strokes, 0);
        glyph.width = glyph
            .strokes
            .iter()
            .flatten()
            .map(|p| p.0)
            .fold(0.0, f64::max);
        glyphs.insert(c, glyph);
    }
    Font {
        glyphs,
        letter_spacing,
        word_spacing,
    }
}

/// The strokes of `c`, with the glyphs it includes (accents on a base
/// letter), guarded against include loops.
fn collect(
    c: char,
    bodies: &HashMap<char, Vec<String>>,
    out: &mut Vec<Vec<(f64, f64, f64)>>,
    depth: u8,
) {
    let Some(body) = bodies.get(&c) else { return };
    if depth > 4 {
        return;
    }
    for line in body {
        if let Some(code) = line.strip_prefix('C') {
            if let Some(inner) = u32::from_str_radix(code, 16).ok().and_then(char::from_u32) {
                collect(inner, bodies, out, depth + 1);
            }
            continue;
        }
        let points: Vec<(f64, f64, f64)> = line
            .split(';')
            .filter_map(|point| {
                let mut parts = point.split(',');
                let x = parts.next()?.trim().parse().ok()?;
                let y = parts.next()?.trim().parse().ok()?;
                let bulge = parts
                    .next()
                    .and_then(|a| a.trim().strip_prefix('A'))
                    .and_then(|b| b.parse().ok())
                    .unwrap_or(0.0);
                Some((x, y, bulge))
            })
            .collect();
        if !points.is_empty() {
            out.push(points);
        }
    }
}

/// A segment in font units.
type Segment = ((f64, f64), (f64, f64));

/// One line of text in font units: its strokes and its advance width.
fn line_strokes(text: &str) -> (Vec<Segment>, f64) {
    let font = font();
    let mut segments = Vec::new();
    let mut x = 0.0;
    let mut last_spacing = 0.0;
    for c in text.chars() {
        if c.is_whitespace() {
            x += font.word_spacing;
            last_spacing = 0.0;
            continue;
        }
        let glyph = font
            .glyphs
            .get(&c)
            .or_else(|| font.glyphs.get(&'\u{fffd}'))
            .or_else(|| font.glyphs.get(&'?'));
        let Some(glyph) = glyph else { continue };
        for stroke in &glyph.strokes {
            push_stroke(stroke, x, &mut segments);
        }
        x += glyph.width + font.letter_spacing;
        last_spacing = font.letter_spacing;
    }
    (segments, (x - last_spacing).max(0.0))
}

/// A glyph's polyline as segments, its arcs flattened, moved `dx` along.
fn push_stroke(stroke: &[(f64, f64, f64)], dx: f64, out: &mut Vec<Segment>) {
    if stroke.len() == 1 {
        let (x, y, _) = stroke[0];
        out.push(((x + dx, y), (x + dx, y))); // a dot
        return;
    }
    for pair in stroke.windows(2) {
        let (a, b) = ((pair[0].0 + dx, pair[0].1), (pair[1].0 + dx, pair[1].1));
        let bulge = pair[1].2;
        if bulge == 0.0 {
            out.push((a, b));
            continue;
        }
        // An arc from a to b whose sweep is 4·atan(bulge), counterclockwise
        // when positive: a circle through both, its center off the chord.
        let sweep = 4.0 * bulge.atan();
        let steps = ((sweep.abs() / (PI / 8.0)).ceil() as usize).max(2);
        let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (cx, cy) = (b.0 - a.0, b.1 - a.1);
        let k = (1.0 - bulge * bulge) / (4.0 * bulge);
        let center = (mx - cy * k, my + cx * k);
        let start = (a.1 - center.1).atan2(a.0 - center.0);
        let radius = ((a.0 - center.0).powi(2) + (a.1 - center.1).powi(2)).sqrt();
        let mut previous = a;
        for step in 1..=steps {
            let next = if step == steps {
                b
            } else {
                let angle = start + sweep * step as f64 / steps as f64;
                (
                    center.0 + radius * angle.cos(),
                    center.1 + radius * angle.sin(),
                )
            };
            out.push((previous, next));
            previous = next;
        }
    }
}

/// The strokes of `frame` and their box, in world coordinates; `None` for
/// a text that cannot be placed (a height that is not positive, numbers
/// that are not finite).
pub(crate) fn layout(frame: &TextFrame) -> Option<Layout> {
    let finite = [
        frame.height,
        frame.rotation,
        frame.width_factor,
        frame.oblique,
        frame.first.0,
        frame.first.1,
    ];
    if !(frame.height > 0.0 && finite.iter().all(|v| v.is_finite())) {
        return None;
    }
    let height = frame.height;
    let mut scale_x = height / CAP * frame.width_factor.abs().max(1e-6);
    let wrapped: Vec<String> = match frame.wrap {
        Some(width) if width > 0.0 && width.is_finite() => frame
            .lines
            .iter()
            .flat_map(|line| wrap_line(line, width / scale_x))
            .collect(),
        _ => frame.lines.clone(),
    };
    let lines: Vec<_> = wrapped.iter().map(|l| line_strokes(l)).collect();
    let mut scale_y = height / CAP;
    let mut rotation = frame.rotation;
    let mut anchor = frame.first;
    let mut h = frame.h;
    // Aligned and Fit fill the span between their two points.
    if let (HAlign::Aligned | HAlign::Fit, Some(second)) = (frame.h, frame.second) {
        let (dx, dy) = (second.0 - frame.first.0, second.1 - frame.first.1);
        let span = (dx * dx + dy * dy).sqrt();
        let width = lines.first().map_or(0.0, |l| l.1);
        if span > 0.0 && width > 0.0 {
            rotation = dy.atan2(dx);
            scale_x = span / width;
            if frame.h == HAlign::Aligned {
                scale_y = scale_x / frame.width_factor.abs().max(1e-6);
            }
        }
        h = HAlign::Left;
    } else if !(frame.h == HAlign::Left && frame.v == VAlign::Baseline) {
        anchor = frame.second.unwrap_or(frame.first);
    }
    let text_height = CAP * scale_y;
    let block = text_height + LINE_SPACING * text_height * (lines.len().max(1) - 1) as f64;
    // The first baseline, from the anchor, by the vertical justification.
    let first_baseline = match (h, frame.v) {
        (HAlign::Middle, _) => -text_height / 2.0,
        (_, VAlign::Baseline) => 0.0,
        (_, VAlign::Bottom) => DESCENDER * scale_y + block - text_height,
        (_, VAlign::Middle) => block / 2.0 - text_height,
        (_, VAlign::Top) => -text_height,
    };
    let shear = frame.oblique.tan();
    let (sin, cos) = rotation.sin_cos();
    let mut strokes = Vec::new();
    let (mut min, mut max) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for (index, (segments, width)) in lines.iter().enumerate() {
        let shift = match h {
            HAlign::Center | HAlign::Middle => -width * scale_x / 2.0,
            HAlign::Right => -width * scale_x,
            _ => 0.0,
        };
        let baseline = first_baseline - index as f64 * LINE_SPACING * text_height;
        let place = |(x, y): (f64, f64)| {
            let mut lx = x * scale_x + y * scale_y * shear + shift;
            let mut ly = y * scale_y + baseline;
            if frame.backward {
                lx = -lx;
            }
            if frame.upside_down {
                ly = -ly;
            }
            let ox = anchor.0 + lx * cos - ly * sin;
            let oy = anchor.1 + lx * sin + ly * cos;
            let (wx, wy, _) = frame.normal.map_point(ox, oy, 0.0);
            (wx, wy)
        };
        for &(a, b) in segments {
            let (a, b) = (place(a), place(b));
            for p in [a, b] {
                min = (min.0.min(p.0), min.1.min(p.1));
                max = (max.0.max(p.0), max.1.max(p.1));
            }
            if let (Ok(a), Ok(b)) = (Point2::new(a.0, a.1), Point2::new(b.0, b.1)) {
                strokes.push((a, b));
            }
        }
    }
    if strokes.is_empty() {
        let (wx, wy, _) = frame.normal.map_point(anchor.0, anchor.1, 0.0);
        let p = Point2::new(wx, wy).ok()?;
        return Some(Layout {
            strokes,
            bounds: Bounds2::new(p, p),
        });
    }
    let bounds = Bounds2::new(
        Point2::new(min.0, min.1).ok()?,
        Point2::new(max.0, max.1).ok()?,
    );
    Some(Layout { strokes, bounds })
}

/// A paragraph broken at spaces into lines no wider than `width` font
/// units; a word wider than that stays whole on a line of its own.
fn wrap_line(text: &str, width: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if current.is_empty() || line_strokes(&candidate).1 <= width {
            current = candidate;
        } else {
            lines.push(std::mem::replace(&mut current, word.to_owned()));
        }
    }
    lines.push(current);
    lines
}

/// The plain lines of MTEXT content: paragraphs (`\P`) become lines; font,
/// height, color and other formatting codes and brace groups are dropped;
/// `\~` is a space and `\\`, `\{`, `\}` the characters themselves; a stacked
/// fraction `\Sa^b;` reads `a/b`.
pub(crate) fn plain_mtext(value: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => match chars.next() {
                Some('P') => lines.push(String::new()),
                Some('~') => lines.last_mut().expect("a line").push(' '),
                Some(c @ ('\\' | '{' | '}')) => lines.last_mut().expect("a line").push(c),
                Some('S') => {
                    let mut stacked = String::new();
                    for c in chars.by_ref() {
                        if c == ';' {
                            break;
                        }
                        stacked.push(if matches!(c, '^' | '#') { '/' } else { c });
                    }
                    lines.last_mut().expect("a line").push_str(&stacked);
                }
                // Codes with a value up to ';': font, height, color, width…
                Some('f' | 'F' | 'H' | 'C' | 'c' | 'T' | 'Q' | 'W' | 'A' | 'p') => {
                    for c in chars.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                }
                // Toggles: underline, overline, strike-through.
                Some(_) | None => {}
            },
            '\n' => lines.push(String::new()),
            c => lines.last_mut().expect("a line").push(c),
        }
    }
    lines
}

#[cfg(test)]
#[path = "text_layout_tests.rs"]
mod tests;
