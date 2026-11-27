//! A light's glyph: the symbol a LIGHT is shown by, as AutoCAD shows it
//! (maintainer, 2026-11-27). A point or web light is a bulb, here three
//! circles of a sphere around it; a spot light a cone from it toward its
//! target, opening by its falloff angle; a distant light has none. All of
//! them hide when the drawing's LIGHTGLYPHDISPLAY is off.
//!
//! A glyph shows where a light is while drawing; it is not part of the
//! drawing on paper, so plotting, when it comes, leaves it out unless the
//! light asks for it (`plot_glyph`, DXF 291), as AutoCAD does.
//!
//! AutoCAD draws a glyph at one size on the screen; here it has a size in
//! the model, [`GLYPH_METRES`] in the drawing's units, so it is placed,
//! picked and fitted in the view like any other entity.

use std::f64::consts::{FRAC_PI_2, TAU};

use acadrust::entities::Light;
use acadrust::types::Vector3;

/// A glyph's size in metres: the bulb's width, the cone's length.
const GLYPH_METRES: f64 = 0.3;

/// Segments a glyph's circle is drawn with.
const SEGMENTS: usize = 32;

/// Light types (DXF 70) as AutoCAD writes them: 10 is what it writes for
/// a web light (Autodesk's condominium sample, 2026-11-27).
const DISTANT: i32 = 1;
const SPOT: i32 = 3;

/// The widest a spot's cone is drawn, half its angle: past it the cone
/// would reach far sideways.
const WIDEST_HALF_ANGLE: f64 = 80.0 * std::f64::consts::PI / 180.0;

/// The polylines of `light`'s glyph in world coordinates; none when it has
/// no glyph or `shown` (LIGHTGLYPHDISPLAY) is off. `size` is the glyph's
/// size in drawing units ([`glyph_size`]).
pub(crate) fn glyph(light: &Light, shown: bool, size: f64) -> Vec<Vec<Vector3>> {
    let at = light.position;
    if !shown || !finite(at) || !(size.is_finite() && size > 0.0) {
        return Vec::new();
    }
    match light.light_type {
        DISTANT => Vec::new(),
        SPOT => {
            let axis = unit(light.target - at).unwrap_or(Vector3::new(0.0, 0.0, -1.0));
            spot(at, axis, light.falloff_angle, size)
        }
        // Point, web, and any other: a bulb.
        _ => bulb(at, size / 2.0),
    }
}

/// The glyph's size in drawing units: [`GLYPH_METRES`] in the units
/// INSUNITS names; in a drawing without units, millimetres when it is
/// metric (MEASUREMENT 1), inches when it is not.
pub(crate) fn glyph_size(insunits: i16, measurement: i16) -> f64 {
    let metres_per_unit = match insunits {
        1 => 0.0254,
        2 => 0.3048,
        3 => 1609.344,
        4 => 0.001,
        5 => 0.01,
        6 => 1.0,
        7 => 1000.0,
        8 => 2.54e-8,
        9 => 2.54e-5,
        10 => 0.9144,
        11 => 1e-10,
        12 => 1e-9,
        13 => 1e-6,
        14 => 0.1,
        15 => 10.0,
        16 => 100.0,
        17 => 1e9,
        18 => 1.495_978_707e11,
        19 => 9.460_730_472_580_8e15,
        20 => 3.085_677_581_491_367e16,
        21 => 1200.0 / 3937.0,
        _ if measurement == 1 => 0.001,
        _ => 0.0254,
    };
    GLYPH_METRES / metres_per_unit
}

/// Three circles of the sphere of `radius` around `at`, one on each
/// world plane.
fn bulb(at: Vector3, radius: f64) -> Vec<Vec<Vector3>> {
    let x = Vector3::new(1.0, 0.0, 0.0);
    let y = Vector3::new(0.0, 1.0, 0.0);
    let z = Vector3::new(0.0, 0.0, 1.0);
    vec![
        circle(at, x * radius, y * radius),
        circle(at, x * radius, z * radius),
        circle(at, y * radius, z * radius),
    ]
}

/// A cone from `apex` along `axis` (a unit vector), `length` long, its
/// whole angle `falloff` (radians): its base circle and four lines from
/// the apex to it.
fn spot(apex: Vector3, axis: Vector3, falloff: f64, length: f64) -> Vec<Vec<Vector3>> {
    let half = if falloff.is_finite() && falloff > 0.0 {
        (falloff / 2.0).min(WIDEST_HALF_ANGLE)
    } else {
        FRAC_PI_2 / 4.0
    };
    let radius = length * half.tan();
    let (u, v) = across(axis);
    let center = apex + axis * length;
    let mut paths = vec![circle(center, u * radius, v * radius)];
    for k in 0..4 {
        let t = k as f64 * FRAC_PI_2;
        let rim = center + u * (radius * t.cos()) + v * (radius * t.sin());
        paths.push(vec![apex, rim]);
    }
    paths
}

/// A circle around `center` through `center + a` and `center + b`, `a`
/// and `b` at right angles and of one length.
fn circle(center: Vector3, a: Vector3, b: Vector3) -> Vec<Vector3> {
    (0..=SEGMENTS)
        .map(|i| {
            let t = TAU * i as f64 / SEGMENTS as f64;
            center + a * t.cos() + b * t.sin()
        })
        .collect()
}

/// Two unit vectors at right angles to `axis` (a unit vector) and to each
/// other.
fn across(axis: Vector3) -> (Vector3, Vector3) {
    let helper = if axis.z.abs() < 0.9 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let u = unit(axis.cross(&helper)).unwrap_or(Vector3::new(1.0, 0.0, 0.0));
    (u, axis.cross(&u))
}

fn unit(v: Vector3) -> Option<Vector3> {
    let length = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    (finite(v) && length > 1e-12).then(|| v * (1.0 / length))
}

fn finite(v: Vector3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

#[cfg(test)]
#[path = "light_glyph_tests.rs"]
mod tests;
