//! Tests of the view a drawing keeps (a submodule of `kept_view`).

use super::*;
use crate::error::DocumentError;

#[test]
fn a_new_drawing_keeps_the_plan() {
    let view = Document::new().kept_view().unwrap();
    assert_eq!((view.direction, view.twist), ([0.0, 0.0, 1.0], 0.0));
}

#[test]
fn a_kept_view_is_saved_and_read_back() {
    let mut doc = Document::new();
    // As AutoCAD may leave it: looking at another point, turned.
    let vport = doc.cad_mut().vports.get_mut(ACTIVE).unwrap();
    vport.view_target = acadrust::types::Vector3::new(5.0, 6.0, 7.0);
    vport.view_twist = 0.3;
    doc.keep_view([-1.0, -1.0, 1.0], (12.5, -3.0), 40.0)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for name in ["vista.dwg", "vista.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let view = Document::open(&path).unwrap().kept_view().unwrap();
        assert_eq!(view.direction, [-1.0, -1.0, 1.0], "{name}");
        assert_eq!(view.target, [0.0, 0.0, 0.0], "{name}");
        assert_eq!(view.twist, 0.0, "{name}");
        assert_eq!((view.center, view.height), ((12.5, -3.0), 40.0), "{name}");
    }
}

#[test]
fn an_impossible_view_is_refused_and_nothing_changes() {
    let mut doc = Document::new();
    let before = doc.kept_view();
    let refused = [
        doc.keep_view([0.0, 0.0, 0.0], (0.0, 0.0), 1.0),
        doc.keep_view([f64::NAN, 0.0, 1.0], (0.0, 0.0), 1.0),
        doc.keep_view([0.0, 0.0, 1.0], (f64::INFINITY, 0.0), 1.0),
        doc.keep_view([0.0, 0.0, 1.0], (0.0, f64::NAN), 1.0),
        doc.keep_view([0.0, 0.0, 1.0], (0.0, 0.0), 0.0),
        doc.keep_view([0.0, 0.0, 1.0], (0.0, 0.0), f64::NAN),
    ];
    assert!(
        refused
            .iter()
            .all(|r| matches!(r, Err(DocumentError::Geometry(_)))),
        "{refused:?}"
    );
    assert_eq!(doc.kept_view(), before);
}

#[test]
fn keeping_a_view_records_no_change() {
    let mut doc = Document::new();
    doc.begin_change().unwrap();
    doc.keep_view([1.0, 0.0, 0.0], (0.0, 0.0), 10.0).unwrap();
    assert!(doc.commit_change().unwrap().summary().is_empty());
}
