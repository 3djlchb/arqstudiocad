//! Tests of the scene seen from a view (a submodule of `scene`): entities
//! projected onto the view's plane, and kept so after changes.

use arq_core::CadDocument;
use arq_geometry::{Point2, Vector2};

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// The ends of every segment, in order.
fn segments(scene: &Scene) -> Vec<((f64, f64), (f64, f64))> {
    scene
        .entries()
        .iter()
        .flat_map(|e| &e.items)
        .filter_map(|i| match i.shape {
            Shape::Line(l) => Some(((l.start().x(), l.start().y()), (l.end().x(), l.end().y()))),
            _ => None,
        })
        .collect()
}

#[test]
fn from_the_front_a_line_in_plan_lies_on_the_ground() {
    let mut cad = CadDocument::new();
    cad.add_line(p(1.0, 2.0), p(5.0, 2.0)).unwrap();
    let plan = Scene::build(cad.document());
    assert_eq!(
        (plan.view(), segments(&plan)),
        (View::Top, vec![((1.0, 2.0), (5.0, 2.0))])
    );
    // Across is X and up is Z: Y runs away from the viewer.
    let front = Scene::build_in(cad.document(), View::Front);
    assert_eq!(segments(&front), [((1.0, 0.0), (5.0, 0.0))]);
    // From the right, across is Y: the line is seen end on, a point.
    let right = Scene::build_in(cad.document(), View::Right);
    let shapes: Vec<_> = right.entries()[0].items.iter().map(|i| i.shape).collect();
    assert_eq!(shapes, [Shape::Point(p(2.0, 0.0))]);
}

#[test]
fn a_scene_keeps_its_view_through_changes() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(1.0, 2.0), p(5.0, 2.0)).unwrap();
    let mut front = Scene::build_in(cad.document(), View::Front);
    // Moved away from the viewer: nothing changes on screen.
    let changes = cad
        .move_entities(&[id], Vector2::new(0.0, 3.0).unwrap())
        .unwrap();
    assert!(front.update(cad.document(), &changes));
    assert_eq!(segments(&front), [((1.0, 0.0), (5.0, 0.0))]);
    let changes = cad
        .move_entities(&[id], Vector2::new(2.0, 0.0).unwrap())
        .unwrap();
    front.update(cad.document(), &changes);
    assert_eq!(segments(&front), [((3.0, 0.0), (7.0, 0.0))]);
    let (second, changes) = cad.add_line(p(0.0, 0.0), p(0.0, 1.0)).unwrap();
    front.update(cad.document(), &changes);
    assert_eq!(front.items(second).map(<[DrawItem]>::len), Some(1));
    // A layer change builds it again, still from the front.
    let layer = cad.document().layers()[0].id();
    let changes = cad.set_layer_visible(layer, false).unwrap();
    front.update(cad.document(), &changes);
    assert_eq!((front.view(), front.shape_count()), (View::Front, 0));
}

#[test]
fn a_text_box_picks_but_is_not_drawn() {
    // A text's box seen from the front: the box around it, as in plan.
    let corner = |x: f64, z: f64| arq_geometry::Point3::new(x, 0.0, z).unwrap();
    let item = SpaceItem {
        points: vec![
            corner(0.0, 0.0),
            corner(4.0, 0.0),
            corner(4.0, 1.0),
            corner(0.0, 0.0),
        ],
        color: arq_core::Color::Index(7),
        text_box: true,
    };
    let shapes: Vec<_> = projected(&[item], View::Front)
        .into_iter()
        .map(|i| i.shape)
        .collect();
    let around = Bounds2::new(p(0.0, 0.0), p(4.0, 1.0));
    assert_eq!(shapes, [Shape::Text(around)]);
}

#[test]
fn picking_and_windows_work_in_a_view() {
    let mut cad = CadDocument::new();
    let (ground, _) = cad.add_line(p(1.0, 2.0), p(5.0, 2.0)).unwrap();
    let (other, _) = cad.add_line(p(10.0, 0.0), p(12.0, 0.0)).unwrap();
    // From the front both lie on the ground line, at x 1 to 5 and 10 to 12.
    let front = Scene::build_in(cad.document(), View::Front);
    assert_eq!(front.pick(p(3.0, 0.1), 0.2), Some(ground));
    assert_eq!(front.pick(p(3.0, 0.5), 0.2), None);
    assert_eq!(front.pick(p(3.0, 0.0), f64::NAN), None);
    assert_eq!(
        front.pick(p(300.0, 0.0), f64::INFINITY),
        None,
        "not anything"
    );
    let around = |a: (f64, f64), b: (f64, f64)| Bounds2::new(p(a.0, a.1), p(b.0, b.1));
    assert_eq!(
        front.select_window(around((0.0, -1.0), (6.0, 1.0)), false),
        [ground]
    );
    assert_eq!(
        front.select_window(around((4.0, -1.0), (11.0, 1.0)), false),
        []
    );
    assert_eq!(
        front.select_window(around((4.0, -1.0), (11.0, 1.0)), true),
        [ground, other]
    );
}
