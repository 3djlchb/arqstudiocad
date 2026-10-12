//! Tests of transactions, groups and undo/redo (a submodule of `history`).

use arq_document::{Entity, Layer, LayerId};
use arq_geometry::{Point2, Vector2};

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

/// Everything a user can observe, for "nothing changed" checks.
fn state(cad: &CadDocument) -> (Vec<Entity>, Vec<Layer>, LayerId) {
    let d = cad.document();
    (d.entities(), d.layers(), d.current_layer().id())
}

#[test]
fn each_operation_is_one_undo_step() {
    let mut cad = CadDocument::new();
    let empty = state(&cad);
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let drawn = state(&cad);
    cad.move_entities(&[id], v(5.0, 0.0)).unwrap();
    let moved = state(&cad);
    assert_eq!(cad.undo_name(), Some("MOVE"));

    assert_eq!(cad.undo().unwrap().modified(), [id]);
    assert_eq!(state(&cad), drawn);
    assert_eq!(cad.undo().unwrap().deleted(), [id]);
    assert_eq!(state(&cad), empty);
    assert!(!cad.can_undo());
    assert_eq!(cad.redo_name(), Some("LINE"));

    assert_eq!(cad.redo().unwrap().created(), [id]);
    assert_eq!(state(&cad), drawn);
    cad.redo().unwrap();
    assert_eq!(state(&cad), moved);
    assert!(!cad.can_redo());
}

#[test]
fn a_new_operation_clears_redo() {
    let mut cad = CadDocument::new();
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    assert!(cad.can_redo());
    cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    assert!(!cad.can_redo());
}

#[test]
fn an_operation_that_changes_nothing_makes_no_step() {
    let mut cad = CadDocument::new();
    let summary = cad.move_entities(&[], v(1.0, 0.0)).unwrap();
    assert!(summary.is_empty());
    assert!(!cad.can_undo());
    // And it does not throw away what could be redone.
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    cad.delete_entities(&[]).unwrap();
    assert!(cad.can_redo());
}

#[test]
fn a_failed_operation_changes_nothing_and_makes_no_step() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let before = state(&cad);
    // `a` moves first, then the missing id fails: `a` must go back.
    let err = cad.move_entities(&[a, 999_999], v(1.0, 1.0)).unwrap_err();
    assert_eq!(err, CadError::EntityNotFound(999_999));
    assert_eq!(state(&cad), before);
    assert_eq!(cad.undo_name(), Some("LINE"));
}

#[test]
fn a_group_is_undone_as_one_step() {
    let mut cad = CadDocument::new();
    let empty = state(&cad);
    cad.begin_group("COPY");
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (b, _) = cad.add_circle(p(5.0, 5.0), 1.0).unwrap();
    cad.move_entities(&[a], v(0.0, 2.0)).unwrap();
    let summary = cad.end_group().unwrap();
    let done = state(&cad);
    assert_eq!(summary.created(), [a, b], "created then moved is created");
    assert!(summary.modified().is_empty());
    assert_eq!(cad.undo_name(), Some("COPY"));

    assert_eq!(cad.undo().unwrap().deleted(), [a, b]);
    assert_eq!(state(&cad), empty);
    assert!(!cad.can_undo(), "one step for the whole group");
    cad.redo().unwrap();
    assert_eq!(state(&cad), done);
}

#[test]
fn nested_groups_make_one_step_named_by_the_outermost() {
    let mut cad = CadDocument::new();
    cad.begin_group("OUTER");
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.begin_group("INNER");
    cad.add_line(p(0.0, 1.0), p(1.0, 1.0)).unwrap();
    assert!(cad.end_group().unwrap().is_empty(), "nested end");
    assert!(cad.in_group());
    assert_eq!(cad.end_group().unwrap().created().len(), 2);
    assert_eq!(cad.undo_name(), Some("OUTER"));
    cad.undo().unwrap();
    assert!(cad.document().entities().is_empty());
    assert!(!cad.can_undo());
}

#[test]
fn a_failure_inside_a_group_rolls_back_the_whole_group() {
    let mut cad = CadDocument::new();
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let before = state(&cad);
    cad.begin_group("CMD");
    cad.begin_group("SUB");
    let (c, _) = cad.add_circle(p(0.0, 0.0), 2.0).unwrap();
    cad.move_entities(&[c], v(3.0, 0.0)).unwrap();
    assert!(cad.delete_entities(&[424_242]).is_err());
    assert_eq!(state(&cad), before);
    assert!(!cad.in_group(), "every level is closed");
    assert!(cad.end_group().is_err());
    assert_eq!(cad.undo_name(), Some("LINE"));
}

#[test]
fn cancelling_a_group_puts_everything_back() {
    let mut cad = CadDocument::new();
    let before = state(&cad);
    cad.begin_group("CMD");
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (layer, _) = cad.create_layer("TEMP").unwrap();
    let undone = cad.cancel_group().unwrap();
    assert_eq!(undone.deleted(), [id]);
    assert_eq!(undone.layers_deleted(), [layer]);
    assert_eq!(state(&cad), before);
    assert!(!cad.can_undo());
    assert!(cad.cancel_group().is_err(), "nothing left to cancel");
}

#[test]
fn an_empty_group_makes_no_step() {
    let mut cad = CadDocument::new();
    cad.begin_group("NOTHING");
    assert!(cad.end_group().unwrap().is_empty());
    assert!(!cad.can_undo());
}

#[test]
fn undo_and_redo_are_refused_inside_a_group() {
    let mut cad = CadDocument::new();
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    cad.begin_group("CMD");
    assert!(matches!(cad.undo(), Err(CadError::InvalidOperation(_))));
    assert!(matches!(cad.redo(), Err(CadError::InvalidOperation(_))));
    cad.end_group().unwrap();
    assert!(cad.can_redo(), "an empty group keeps redo");
}

#[test]
fn nothing_to_undo_or_redo_is_an_error() {
    let mut cad = CadDocument::new();
    assert!(matches!(cad.undo(), Err(CadError::InvalidOperation(_))));
    assert!(matches!(cad.redo(), Err(CadError::InvalidOperation(_))));
    assert_eq!((cad.undo_name(), cad.redo_name()), (None, None));
}

#[test]
fn ids_are_not_reused_after_undo() {
    let mut cad = CadDocument::new();
    let (first, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    let (second, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert_ne!(first, second);
}

#[test]
fn saved_files_follow_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut cad = CadDocument::new();
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (c, _) = cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    cad.undo().unwrap();
    for ext in ["dwg", "dxf"] {
        let path = dir.path().join(format!("undone.{ext}"));
        cad.save(&path).unwrap();
        let back = CadDocument::open(&path).unwrap();
        assert_eq!(back.document().entities().len(), 1, "{ext}");
        assert!(back.document().entity(c).is_none(), "{ext}");
    }
}

#[test]
fn a_group_is_undone_in_reverse_order() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let start = state(&cad);
    cad.begin_group("TWICE");
    cad.move_entities(&[id], v(1.0, 0.0)).unwrap();
    cad.move_entities(&[id], v(0.0, 1.0)).unwrap();
    let summary = cad.end_group().unwrap();
    assert_eq!(summary.modified(), [id]);
    let end = state(&cad);
    cad.undo().unwrap();
    assert_eq!(state(&cad), start);
    cad.redo().unwrap();
    assert_eq!(state(&cad), end);
}

#[test]
fn modified_follows_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dxf");
    let mut cad = CadDocument::new();
    assert!(!cad.is_modified(), "a new drawing");
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert!(cad.is_modified());
    cad.undo().unwrap();
    assert!(!cad.is_modified(), "back to the starting state");
    cad.redo().unwrap();
    cad.save(&path).unwrap();
    assert!(!cad.is_modified(), "just saved");
    cad.undo().unwrap();
    assert!(cad.is_modified(), "before the saved state");
    cad.redo().unwrap();
    assert!(!cad.is_modified(), "redo reaches the saved state again");
    cad.undo().unwrap();
    cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    cad.undo().unwrap();
    assert!(
        cad.is_modified(),
        "the saved state cannot be reached any more"
    );
    assert!(
        !CadDocument::open(&path).unwrap().is_modified(),
        "just opened"
    );
}

#[test]
fn an_open_group_with_changes_is_a_modification() {
    let mut cad = CadDocument::new();
    cad.begin_group("CMD");
    assert!(!cad.is_modified());
    cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert!(cad.is_modified());
    cad.cancel_group().unwrap();
    assert!(!cad.is_modified());
}

#[test]
fn a_snapshot_marks_saved_only_its_own_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dwg");
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let snap = cad.snapshot();
    let (b, _) = cad.add_circle(p(0.0, 0.0), 1.0).unwrap(); // during the save
    assert!(snap.document().entity(b).is_none());
    snap.save(&path).unwrap();
    cad.mark_saved(&snap);
    assert!(cad.is_modified(), "the circle is not in the file");
    cad.undo().unwrap();
    assert!(!cad.is_modified(), "now it matches the file");
    let back = CadDocument::open(&path).unwrap();
    assert!(back.document().entity(a).is_some());
    assert!(back.document().entity(b).is_none());
}
