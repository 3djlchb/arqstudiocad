//! Tests of the document store (a submodule of `app`).

use std::thread;

use arq_geometry::Point2;

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// The store is shared with Python and background work: it must cross
/// threads, and so must snapshots.
#[test]
fn the_store_and_snapshots_cross_threads() {
    fn shareable<T: Send + Sync>() {}
    fn movable<T: Send>() {}
    shareable::<CadApplication>();
    movable::<Snapshot>();
}

#[test]
fn documents_are_created_listed_and_closed() {
    let app = CadApplication::new();
    let a = app.create_document("Plano1");
    let b = app.create_document("Plano2");
    assert_eq!(app.document_ids(), [a, b]);
    assert_eq!(app.document_name(b).unwrap(), "Plano2");
    assert_eq!(app.document_path(a).unwrap(), None);
    app.close_document(a).unwrap();
    assert_eq!(app.document_ids(), [b]);
    let c = app.create_document("Plano3");
    assert!(c != a && c != b, "ids are never reused");
}

#[test]
fn an_unknown_document_is_an_error_everywhere() {
    let app = CadApplication::new();
    let gone = app.create_document("x");
    app.close_document(gone).unwrap();
    let missing = CadError::DocumentNotFound(gone);
    assert_eq!(app.close_document(gone).unwrap_err(), missing);
    assert_eq!(app.document_name(gone).unwrap_err(), missing);
    assert_eq!(app.document_path(gone).unwrap_err(), missing);
    assert_eq!(app.read(gone, |_| ()).unwrap_err(), missing);
    assert_eq!(app.edit(gone, |_| Ok(())).unwrap_err(), missing);
    assert!(app.snapshot(gone).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.dxf");
    assert_eq!(app.save_document(gone, &path).unwrap_err(), missing);
    assert!(!path.exists());
}

#[test]
fn save_and_open_carry_name_path_and_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vivienda.dwg");
    let app = CadApplication::new();
    let id = app.create_document("Plano1");
    let (line, _) = app
        .edit(id, |cad| cad.add_line(p(0.0, 0.0), p(4.0, 3.0)))
        .unwrap();
    assert!(app.read(id, CadDocument::is_modified).unwrap());

    app.save_document(id, &path).unwrap();
    assert_eq!(app.document_name(id).unwrap(), "vivienda.dwg");
    assert_eq!(app.document_path(id).unwrap(), Some(path.clone()));
    assert!(!app.read(id, CadDocument::is_modified).unwrap());

    let other = app.open_document(&path).unwrap();
    assert_eq!(app.document_name(other).unwrap(), "vivienda.dwg");
    assert!(
        app.read(other, |cad| cad.document().entity(line).is_some())
            .unwrap()
    );
    assert!(matches!(
        app.open_document(dir.path().join("no-existe.dwg")),
        Err(CadError::Io(_))
    ));
    assert!(matches!(
        app.save_document(id, dir.path().join("plano.pdf")),
        Err(CadError::UnsupportedFormat(_))
    ));
    assert_eq!(app.document_name(id).unwrap(), "vivienda.dwg", "unchanged");
}

/// An AutoCAD 2000 drawing of 300 lines saved at `path`, its bytes changed
/// by `damage` (ADR 0017 D5.4: the regression drawing is made here).
fn damaged_drawing(path: &Path, damage: impl FnOnce(&mut Vec<u8>)) {
    let mut doc = arq_document::Document::new();
    assert!(doc.set_version_for_tests("AC1015"));
    let layer = doc.current_layer().id();
    for i in 0..300 {
        let x = f64::from(i);
        let line = arq_geometry::Line::new(p(x, 0.0), p(x, 10.0)).unwrap();
        let plan = arq_document::PlanGeometry::at_elevation(line.into(), 0.0).unwrap();
        doc.add_entity(plan, layer).unwrap();
    }
    doc.save(path).unwrap();
    let mut bytes = std::fs::read(path).unwrap();
    damage(&mut bytes);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_damaged_file_with_nothing_to_recover_does_not_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cortado.dwg");
    // Cut short, an R2000 DWG once read as an empty drawing.
    damaged_drawing(&path, |b| b.truncate(b.len() / 2));
    let app = CadApplication::new();
    let error = app.open_document(&path).unwrap_err();
    assert!(matches!(error, CadError::DamagedFile(_)), "{error:?}");
    assert!(error.to_string().contains("damaged"), "{error}");
    assert!(app.document_ids().is_empty());
}

#[test]
fn a_recovered_drawing_is_not_saved_over_its_damaged_file_unasked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("danado.dwg");
    damaged_drawing(&path, |b| {
        let m = b.len() / 2;
        b[m..m + 64].iter_mut().for_each(|x| *x ^= 0x5a);
    });
    let original = std::fs::read(&path).unwrap();
    let app = CadApplication::new();
    let id = app.open_document(&path).unwrap();
    let problems = app
        .read(id, |cad| {
            cad.document().recovery().map(|r| r.problems().to_vec())
        })
        .unwrap();
    assert!(
        problems.is_some_and(|p| !p.is_empty()),
        "marked as recovered"
    );

    let refused = app.save_document(id, &path).unwrap_err();
    assert!(
        matches!(refused, CadError::OverwritesDamaged(_)),
        "{refused:?}"
    );
    // The same file under another spelling is refused too.
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let spelled = dir.path().join("sub").join("..").join("danado.dwg");
    assert!(matches!(
        app.save_document(id, &spelled),
        Err(CadError::OverwritesDamaged(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original, "untouched");

    let copy = dir.path().join("recuperado.dwg");
    app.save_document(id, &copy).unwrap();
    assert!(
        app.save_document(id, &path).is_err(),
        "saving elsewhere does not lift the guard"
    );
    app.allow_overwriting_damaged(id).unwrap();
    app.save_document(id, &path).unwrap();
    let back = app.open_document(&path).unwrap();
    assert!(
        app.read(back, |cad| cad.document().recovery().is_none())
            .unwrap()
    );
}

#[test]
fn concurrent_edits_form_one_stream() {
    const THREADS: usize = 8;
    const EACH: usize = 25;
    let app = CadApplication::new();
    let id = app.create_document("Plano1");
    thread::scope(|s| {
        for t in 0..THREADS {
            let app = &app;
            s.spawn(move || {
                for i in 0..EACH {
                    let y = (t * EACH + i) as f64;
                    app.edit(id, |cad| cad.add_line(p(0.0, y), p(1.0, y)))
                        .unwrap();
                }
            });
        }
    });
    let total = THREADS * EACH;
    assert_eq!(
        app.read(id, |cad| cad.document().entities().len()).unwrap(),
        total
    );
    // Every edit is its own undo step: undoing all of them empties the drawing.
    app.edit(id, |cad| {
        for _ in 0..total {
            cad.undo()?;
        }
        Ok(())
    })
    .unwrap();
    assert!(
        app.read(id, |cad| cad.document().entities().is_empty())
            .unwrap()
    );
}

#[test]
fn a_snapshot_is_saved_on_another_thread_while_drawing_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("copia.dxf");
    let app = CadApplication::new();
    let id = app.create_document("Plano1");
    app.edit(id, |cad| cad.add_circle(p(0.0, 0.0), 1.0))
        .unwrap();
    let snapshot = app.snapshot(id).unwrap();
    let saver = thread::spawn({
        let path = path.clone();
        move || snapshot.save(path)
    });
    app.edit(id, |cad| cad.add_line(p(0.0, 0.0), p(1.0, 1.0)))
        .unwrap();
    saver.join().unwrap().unwrap();
    assert_eq!(
        CadDocument::open(&path)
            .unwrap()
            .document()
            .entities()
            .len(),
        1
    );
    assert_eq!(
        app.read(id, |cad| cad.document().entities().len()).unwrap(),
        2
    );
}

#[test]
fn a_panic_while_editing_disables_only_that_document() {
    let app = CadApplication::new();
    let broken = app.create_document("roto");
    let fine = app.create_document("bien");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.edit(broken, |_| -> CadResult<()> { panic!("a bug") })
    }));
    assert!(result.is_err());
    assert!(matches!(
        app.read(broken, |_| ()),
        Err(CadError::InvalidOperation(_))
    ));
    app.edit(fine, |cad| cad.add_line(p(0.0, 0.0), p(1.0, 0.0)))
        .unwrap();
    app.close_document(broken).unwrap();
    assert_eq!(app.document_ids(), [fine]);
}
