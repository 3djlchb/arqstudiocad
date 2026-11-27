//! Tests of lights' glyphs (a submodule of `light_glyph`): their shapes
//! worked out by hand, and how a drawing shows them.

use acadrust::entities::Light;
use acadrust::types::Vector3;
use acadrust::{CadDocument, EntityType};

use super::{glyph, glyph_size};
use crate::document::Document;
use crate::drawing::Shape;

fn light(kind: i32, at: [f64; 3], target: [f64; 3]) -> Light {
    let mut l = Light::new();
    l.light_type = kind;
    l.position = Vector3::new(at[0], at[1], at[2]);
    l.target = Vector3::new(target[0], target[1], target[2]);
    l.falloff_angle = std::f64::consts::FRAC_PI_2; // 90°: a cone as wide as long
    l
}

/// The rounded box of the glyph's points, `[min, max]`.
fn extent(paths: &[Vec<Vector3>]) -> [[f64; 3]; 2] {
    let r = |v: f64| (v * 1e6).round() / 1e6 + 0.0;
    let mut b = [[f64::MAX; 3], [f64::MIN; 3]];
    for p in paths.iter().flatten() {
        for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            b[0][i] = b[0][i].min(v);
            b[1][i] = b[1][i].max(v);
        }
    }
    b.map(|c| c.map(r))
}

#[test]
fn a_point_light_is_a_bulb_around_it() {
    let paths = glyph(&light(2, [10.0, 5.0, 3.0], [10.0, 5.0, 0.0]), true, 2.0);
    // Three circles of radius 1, one on each world plane.
    assert_eq!(paths.len(), 3);
    assert_eq!(extent(&paths), [[9.0, 4.0, 2.0], [11.0, 6.0, 4.0]]);
    // A web light (AutoCAD's type 10) is drawn the same.
    assert_eq!(
        glyph(&light(10, [10.0, 5.0, 3.0], [0.0; 3]), true, 2.0),
        paths
    );
}

#[test]
fn a_spot_light_is_a_cone_toward_its_target() {
    // Pointing down from (0,0,10), 2 long, 90° wide: its base circle of
    // radius 2 at z = 8, and four lines from the apex.
    let paths = glyph(&light(3, [0.0, 0.0, 10.0], [0.0, 0.0, 0.0]), true, 2.0);
    assert_eq!(paths.len(), 5);
    assert_eq!(extent(&paths), [[-2.0, -2.0, 8.0], [2.0, 2.0, 10.0]]);
    // Pointing along x: the cone reaches x = 2 and spreads in y and z.
    let paths = glyph(&light(3, [0.0, 0.0, 0.0], [5.0, 0.0, 0.0]), true, 2.0);
    assert_eq!(extent(&paths), [[0.0, -2.0, -2.0], [2.0, 2.0, 2.0]]);
}

#[test]
fn a_distant_light_or_hidden_glyphs_draw_nothing() {
    assert!(glyph(&light(1, [0.0; 3], [0.0, 0.0, -1.0]), true, 2.0).is_empty());
    assert!(glyph(&light(2, [0.0; 3], [0.0; 3]), false, 2.0).is_empty());
    assert!(glyph(&light(2, [f64::NAN, 0.0, 0.0], [0.0; 3]), true, 2.0).is_empty());
}

#[test]
fn a_glyph_is_a_third_of_a_metre_in_the_drawings_units() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9 * b;
    assert!(close(glyph_size(4, 0), 300.0)); // millimetres
    assert!(close(glyph_size(6, 0), 0.3)); // metres
    assert!(close(glyph_size(1, 0), 0.3 / 0.0254)); // inches
    // Without units: millimetres in a metric drawing, else inches.
    assert!(close(glyph_size(0, 1), 300.0));
    assert!(close(glyph_size(0, 0), 0.3 / 0.0254));
}

/// A drawing in metres holding `l`, and its id.
fn drawing_with(l: Light) -> (Document, u64) {
    let mut cad = CadDocument::new();
    cad.header.insertion_units = 6;
    let h = cad.add_entity(EntityType::Light(Box::new(l))).unwrap();
    (Document::from_engine(cad), h.value())
}

/// The rounded plan box of what is drawn for `id`, and whether all of it
/// is lines.
fn drawn(doc: &Document, id: u64) -> Option<([f64; 4], bool)> {
    let items = doc.drawing(id).unwrap();
    let lines = items.iter().all(|i| matches!(i.shape, Shape::Line(_)));
    let b = items
        .iter()
        .map(|i| i.shape.bounds())
        .reduce(|a, b| a.union(b))?;
    let r = |v: f64| (v * 1e6).round() / 1e6 + 0.0;
    Some((
        [
            r(b.min().x()),
            r(b.min().y()),
            r(b.max().x()),
            r(b.max().y()),
        ],
        lines,
    ))
}

#[test]
fn a_drawing_shows_a_light_by_its_glyph() {
    // A point light at (10, 5) in a drawing in metres: a bulb 0.3 wide,
    // drawn in lines, no box; in space, its circles at its height.
    let (doc, id) = drawing_with(light(2, [10.0, 5.0, 3.0], [10.0, 5.0, 0.0]));
    assert_eq!(drawn(&doc, id), Some(([9.85, 4.85, 10.15, 5.15], true)));
    let space = doc.space_drawing(id).unwrap();
    assert_eq!(space.len(), 3);
    let top = space
        .iter()
        .flat_map(|i| &i.points)
        .map(|p| p.z())
        .fold(f64::MIN, f64::max);
    assert!((top - 3.15).abs() < 1e-9, "{top}");
    // The entity's box, which picks it and fits the view, is the glyph's.
    let b = doc.entity(id).unwrap().bounds().unwrap();
    assert!((b.max().x() - 10.15).abs() < 1e-9, "{b:?}");
}

#[test]
fn lightglyphdisplay_off_hides_every_glyph() {
    let (mut doc, id) = drawing_with(light(2, [10.0, 5.0, 3.0], [10.0, 5.0, 0.0]));
    doc.cad_mut().header.light_glyph_display = 0;
    assert_eq!(drawn(&doc, id), None);
    assert!(doc.space_drawing(id).unwrap().is_empty());
}

#[test]
fn a_light_in_a_block_is_drawn_where_the_block_places_it() {
    // A point light at the block's base, placed at (20, 0).
    let (mut doc, id) = drawing_with(light(2, [0.0, 0.0, 3.0], [0.0, 0.0, 0.0]));
    let base = arq_geometry::Point2::new(0.0, 0.0).unwrap();
    let first = doc.create_block("LAMPARA", base, &[id]).unwrap();
    let at = arq_geometry::Point2::new(20.0, 0.0).unwrap();
    let placed = doc.insert_block("LAMPARA", at, 1.0, 0.0).unwrap();
    assert_eq!(drawn(&doc, first), Some(([-0.15, -0.15, 0.15, 0.15], true)));
    assert_eq!(
        drawn(&doc, placed),
        Some(([19.85, -0.15, 20.15, 0.15], true))
    );
}
