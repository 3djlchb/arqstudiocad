//! Compare indexed Quick to independent exhaustive IDs through Standard's path.

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn equivalent(mut indexed: CadDocument, target: EntityId, at: Point2, extending: bool) {
    let before = indexed.document().entities();
    let mut exhaustive = CadDocument::from_document(indexed.document().snapshot());
    // Deliberately enumerate drawings, never obtain IDs through the new index.
    let all: Vec<_> = exhaustive
        .document()
        .drawing_all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let a = if extending {
        indexed
            .extend_entity(target, at, None)
            .map(|c| (Vec::new(), c))
    } else {
        indexed.trim_entity(target, at, None, true)
    };
    let b = if extending {
        exhaustive
            .extend_entity(target, at, Some(&all))
            .map(|c| (Vec::new(), c))
    } else {
        exhaustive.trim_entity(target, at, Some(&all), true)
    };
    assert!(
        b.is_ok(),
        "the exhaustive fixture must perform a real edit: {b:?}"
    );
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!(
        indexed.document().entities(),
        exhaustive.document().entities()
    );
    assert_eq!(
        indexed.document().drawing_all(),
        exhaustive.document().drawing_all()
    );
    assert_eq!(indexed.undo_name(), exhaustive.undo_name());
    indexed.undo().unwrap();
    exhaustive.undo().unwrap();
    assert_eq!(indexed.document().entities(), before);
    assert_eq!(
        indexed.document().entities(),
        exhaustive.document().entities()
    );
    indexed.redo().unwrap();
    exhaustive.redo().unwrap();
    assert_eq!(
        indexed.document().entities(),
        exhaustive.document().entities()
    );
    let area = arq_geometry::Bounds2::new(p(-1e7, -1e7), p(1e7, 1e7));
    assert_eq!(
        indexed.query_entities(area),
        exhaustive.query_entities(area)
    );
}

fn cutters(cad: &mut CadDocument, a: f64, b: f64) {
    for x in [a, b] {
        cad.add_line(p(x, -1e6), p(x, 1e6)).unwrap();
    }
    for i in 0..25 {
        cad.add_line(p(1000. + i as f64, 100.), p(1001. + i as f64, 101.))
            .unwrap();
    }
}

#[test]
fn quick_trim_index_matches_exhaustive_long_lines_round_curves_blocks_and_layers() {
    for kind in 0..4 {
        let mut cad = CadDocument::new();
        let (target, at) = match kind {
            0 | 3 => (cad.add_line(p(0., 0.), p(10., 0.)).unwrap().0, p(5., 0.)),
            1 => (cad.add_circle(p(0., 0.), 5.).unwrap().0, p(0., 5.)),
            _ => (
                cad.add_arc(p(0., 0.), 5., 0., std::f64::consts::PI)
                    .unwrap()
                    .0,
                p(0., 5.),
            ),
        };
        if kind == 3 {
            let (a, _) = cad.add_line(p(3., -1.), p(3., 1.)).unwrap();
            let (b, _) = cad.add_line(p(7., -1.), p(7., 1.)).unwrap();
            cad.create_block("cutters", p(1e6, 1e6), &[a, b]).unwrap();
        } else {
            cutters(
                &mut cad,
                if kind == 0 { 3. } else { -3. },
                if kind == 0 { 7. } else { 3. },
            );
        }
        let (hidden, _) = cad.create_layer("hidden").unwrap();
        let (line, _) = cad.add_line(p(4., -100.), p(4., 100.)).unwrap();
        cad.set_entity_layer(&[line], hidden).unwrap();
        cad.set_layer_visible(hidden, false).unwrap();
        cad.set_entity_color(&[target], Color::Index(3)).unwrap();
        if kind == 0 {
            let candidates = cad
                .quick_edge_candidates(target, at, false)
                .expect("indexed query");
            assert!(candidates.len() < cad.document().entities().len());
        }
        equivalent(cad, target, at, false);
    }
    for round in [false, true] {
        let mut cad = CadDocument::new();
        let (target, _) = cad.add_line(p(0., 0.), p(10., 0.)).unwrap();
        if round {
            cad.add_circle(p(5., 0.), 2.).unwrap();
        } else {
            cad.add_arc(p(5., 0.), 2., 0., std::f64::consts::PI)
                .unwrap();
        }
        equivalent(cad, target, p(5., 0.), false);
    }
}

#[test]
fn quick_extend_index_matches_exhaustive_far_rays_both_ends_and_arc_circles() {
    for end in [false, true] {
        let mut cad = CadDocument::new();
        let (target, _) = cad.add_line(p(0., 0.), p(10., 0.)).unwrap();
        cutters(&mut cad, -1000., 1000.);
        cad.add_line(p(20., 10.), p(20., 11.)).unwrap(); // nearer box, outside ray
        equivalent(cad, target, if end { p(9., 0.) } else { p(1., 0.) }, true);
    }
    let mut cad = CadDocument::new();
    let (target, _) = cad
        .add_arc(p(0., 0.), 5., 0., std::f64::consts::FRAC_PI_2)
        .unwrap();
    cad.add_line(p(-3., 2.), p(-3., 6.)).unwrap(); // outside the original arc box
    equivalent(cad, target, p(0., 5.), true);
    let mut cad = CadDocument::new();
    let (target, _) = cad.add_line(p(0., 0.), p(10., 10.)).unwrap();
    cad.add_line(p(1000., 0.), p(1000., 2000.)).unwrap();
    equivalent(cad, target, p(9., 9.), true);
    for round in [false, true] {
        let mut cad = CadDocument::new();
        let (target, _) = cad.add_line(p(0., 0.), p(10., 0.)).unwrap();
        if round {
            cad.add_circle(p(1000., 0.), 5.).unwrap();
        } else {
            cad.add_arc(
                p(1000., 0.),
                5.,
                std::f64::consts::FRAC_PI_2,
                3. * std::f64::consts::FRAC_PI_2,
            )
            .unwrap();
        }
        equivalent(cad, target, p(9., 0.), true);
    }
}

#[test]
fn quick_index_matches_exhaustive_polylines_curved_ends_and_mirrored_planes() {
    for bulge in [0., 1., -1.] {
        let mut cad = CadDocument::new();
        let (target, _) = cad
            .add_polyline(
                &[(p(0., 0.), 0.), (p(10., 0.), bulge), (p(10., 10.), 0.)],
                false,
            )
            .unwrap();
        cad.add_line(p(5., -20.), p(5., 30.)).unwrap();
        cad.add_line(p(15., -20.), p(15., 30.)).unwrap();
        cad.add_line(p(0., 15.), p(20., 15.)).unwrap();
        equivalent(cad, target, p(10., 10.), true);
    }
    for closed in [false, true] {
        let mut cad = CadDocument::new();
        let (target, _) = cad
            .add_polyline(
                &[(p(0., 0.), 0.5), (p(10., 0.), 0.), (p(10., 10.), 0.)],
                closed,
            )
            .unwrap();
        cutters(&mut cad, 3., 7.);
        equivalent(cad, target, p(5., -2.), false);
    }
    let text = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n3\n70\n0\n210\n0\n220\n0\n230\n-1\n10\n0\n20\n0\n42\n0\n10\n10\n20\n0\n42\n-1\n10\n10\n20\n10\n0\nENDSEC\n0\nEOF\n";
    let doc = Document::from_dxf_bytes(text.as_bytes().to_vec()).unwrap();
    let mut cad = CadDocument::from_document(doc);
    let target = cad.document().entities()[0].id();
    cad.add_line(p(-5., -20.), p(-5., 30.)).unwrap();
    cad.add_line(p(-15., -20.), p(-15., 30.)).unwrap();
    equivalent(cad, target, p(-10., 10.), true);
}

#[test]
fn quick_index_preserves_relative_slack_and_updated_index_after_undo() {
    let mut cad = CadDocument::new();
    let (target, _) = cad.add_line(p(1e6, 0.), p(1e6 + 10., 0.)).unwrap();
    // EXTEND's existing relative on-edge slack admits the tiny gap to y=0.
    let (edge, _) = cad
        .add_line(p(1e6 + 20., 0.0001), p(1e6 + 20., 2.))
        .unwrap();
    cad.move_entities(&[edge], Vector2::new(0., 50.).unwrap())
        .unwrap();
    cad.undo().unwrap();
    equivalent(cad, target, p(1e6 + 9., 0.), true);
    let mut cad = CadDocument::new();
    let (target, _) = cad.add_circle(p(0., 0.), 5.).unwrap();
    cad.add_circle(p(10. + 0.5e-9, 0.), 5.).unwrap(); // tolerance-level tangency
    equivalent(cad, target, p(5., 0.), false);
    let arc = Arc::new(
        p(1e308, 0.),
        1e308,
        std::f64::consts::PI,
        3. * std::f64::consts::FRAC_PI_2,
    )
    .unwrap();
    let extent = arq_geometry::Bounds2::new(p(0., -1e308), p(1e308, 0.));
    assert!(
        extension_query_area(arc.into(), p(0., 0.), extent, 1e299).is_none(),
        "overflow must use exhaustive fallback"
    );
}
