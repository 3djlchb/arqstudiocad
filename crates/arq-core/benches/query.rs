//! Queries, snapshots and transactions on a 100,000-entity drawing (contract §12, ADR 0011 D3.8).
//!
//! Measures spatial queries and indexed interaction candidates on a
//! 100,000-entity drawing: `cargo bench -p arq-core`. A reproducible target,
//! not a promise.

use std::hint::black_box;

use arq_core::{CadDocument, Document, PlanGeometry, SnapKind};
use arq_geometry::{Arc, Bounds2, Circle, Geometry2, Line, Point2};
use criterion::{Criterion, criterion_group, criterion_main};

/// Side of the square grid: 317² ≈ 100,000 cells, one entity each.
const SIDE: usize = 317;
/// Distance between cell centers, in model units.
const STEP: f64 = 10.0;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).expect("finite")
}

/// A grid of lines, circles and arcs, built without transactions.
fn drawing() -> CadDocument {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    for i in 0..SIDE * SIDE {
        let (x, y) = ((i % SIDE) as f64 * STEP, (i / SIDE) as f64 * STEP);
        let g: Geometry2 = match i % 3 {
            0 => Line::new(p(x, y), p(x + 6.0, y + 3.0))
                .expect("line")
                .into(),
            1 => Circle::new(p(x + 3.0, y + 3.0), 2.5)
                .expect("circle")
                .into(),
            _ => Arc::new(p(x + 3.0, y + 3.0), 2.5, 0.3, 2.8)
                .expect("arc")
                .into(),
        };
        doc.add_entity(PlanGeometry::new(g), layer).expect("add");
    }
    CadDocument::from_document(doc)
}

fn queries(c: &mut Criterion) {
    let mut cad = drawing();
    let extent = SIDE as f64 * STEP;
    // About 1% of the drawing, like a selection window.
    let window = Bounds2::new(
        p(0.45 * extent, 0.45 * extent),
        p(0.55 * extent, 0.55 * extent),
    );
    let all = Bounds2::new(p(-1.0, -1.0), p(extent, extent));

    let mut group = c.benchmark_group("100k entities");
    group.sample_size(20);
    group.bench_function("bounds", |b| b.iter(|| black_box(cad.bounds())));
    group.bench_function("query 1% window", |b| {
        b.iter(|| black_box(cad.query_entities(black_box(window))))
    });
    group.bench_function("query everything", |b| {
        b.iter(|| black_box(cad.query_entities(black_box(all))))
    });
    // Hover and selection (ADR 0016 D2, D5): the budget is one frame on
    // the reference machine for a few thousand entities; this records 100k.
    // The cursor lies on the first line so pick and snap measure a hit, not
    // only the cost of an empty broad-phase query.
    let cursor = p(4.0, 2.0);
    group.bench_function("pick", |b| {
        b.iter(|| black_box(cad.pick(black_box(cursor), 1.0).expect("pick")))
    });
    group.bench_function("select 1% window", |b| {
        b.iter(|| black_box(cad.select_window(black_box(window), false)))
    });
    group.bench_function("select 1% crossing", |b| {
        b.iter(|| black_box(cad.select_window(black_box(window), true)))
    });
    let modes = [
        SnapKind::Endpoint,
        SnapKind::Midpoint,
        SnapKind::Center,
        SnapKind::Intersection,
    ];
    group.bench_function("snap (default modes)", |b| {
        b.iter(|| {
            black_box(
                cad.snap(black_box(cursor), 2.0, &modes, None)
                    .expect("snap"),
            )
        })
    });
    // What saving and autosave hold the document lock for (D3.10).
    group.bench_function("snapshot", |b| b.iter(|| black_box(cad.snapshot())));
    // Transaction overhead in a large drawing: one recorded change and its undo.
    group.bench_function("add line + undo", |b| {
        b.iter(|| {
            cad.add_line(p(-5.0, -5.0), p(-1.0, -1.0)).expect("add");
            cad.undo().expect("undo")
        })
    });
    group.finish();
}

/// N unrelated short lines, one target and two possible boundaries.
fn quick_drawing(count: usize, extending: bool) -> (CadDocument, u64, Point2) {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    for i in 0..count {
        let x = 1000.0 + (i % 317) as f64 * 10.0;
        let y = 1000.0 + (i / 317) as f64 * 10.0;
        doc.add_entity(
            PlanGeometry::new(Line::new(p(x, y), p(x + 1.0, y + 1.0)).unwrap().into()),
            layer,
        )
        .unwrap();
    }
    let target = doc
        .add_entity(
            PlanGeometry::new(Line::new(p(0., 0.), p(10., 0.)).unwrap().into()),
            layer,
        )
        .unwrap();
    let positions = if extending { [20., 30.] } else { [3., 7.] };
    for x in positions {
        doc.add_entity(
            PlanGeometry::new(Line::new(p(x, -1.), p(x, 1.)).unwrap().into()),
            layer,
        )
        .unwrap();
    }
    (
        CadDocument::from_document(doc),
        target,
        if extending { p(9., 0.) } else { p(5., 0.) },
    )
}

fn quick_edits(c: &mut Criterion) {
    use criterion::BenchmarkId;
    use std::time::{Duration, Instant};
    let mut group = c.benchmark_group("TRIM EXTEND Quick");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for count in [10_000, 100_000] {
        for extending in [false, true] {
            let (mut cad, target, pick) = quick_drawing(count, extending);
            assert_eq!(cad.document().entities().len(), count + 3);
            let name = if extending { "extend" } else { "trim" };
            group.bench_function(BenchmarkId::new(name, count), |b| {
                b.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let start = Instant::now();
                        if extending {
                            black_box(cad.extend_entity(target, pick, None).expect("extends"));
                        } else {
                            let (ids, changes) =
                                cad.trim_entity(target, pick, None, true).expect("trims");
                            black_box(changes);
                            assert_eq!(ids.len(), 2);
                        }
                        elapsed += start.elapsed();
                        let geometry = cad
                            .document()
                            .entity(target)
                            .unwrap()
                            .plan()
                            .unwrap()
                            .geometry();
                        assert_eq!(
                            geometry.end_point(),
                            Some(if extending { p(20., 0.) } else { p(3., 0.) })
                        );
                        cad.undo().expect("restore outside timing");
                        assert_eq!(
                            cad.document()
                                .entity(target)
                                .unwrap()
                                .plan()
                                .unwrap()
                                .geometry()
                                .end_point(),
                            Some(p(10., 0.))
                        );
                    }
                    elapsed
                })
            });
        }
    }
    group.finish();
}

/// A crossing polygon of 402 corners: a tooth around each line end
/// `(3k+1, 0)`, rising from a base below the row.
fn comb() -> Vec<Point2> {
    let mut points = Vec::new();
    for k in 0..100 {
        let e = f64::from(k) * 3.0 + 1.0;
        points.extend([
            p(e - 0.1, -0.2),
            p(e - 0.1, 0.2),
            p(e + 0.1, 0.2),
            p(e + 0.1, -0.2),
        ]);
    }
    points.extend([p(298.2, -0.4), p(0.8, -0.4)]);
    points
}

/// STRETCH with one crossing polygon over 100 selected lines, given once
/// per entity as the viewport gives it: validating the polygon for every
/// pair against once for all (`selection_polygons`), and the preview.
fn stretch_polygon(c: &mut Criterion) {
    use arq_geometry::{SelectionPolygon, Tolerance, Vector2, selection_polygons};
    use std::time::Duration;
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let mut ids = Vec::new();
    for i in 0..100_000_u32 {
        let (x, y) = (f64::from(i % 1000) * 3.0, f64::from(i / 1000) * 3.0);
        let line = Line::new(p(x, y), p(x + 1.0, y)).expect("line");
        let id = doc
            .add_entity(PlanGeometry::new(line.into()), layer)
            .expect("add");
        if i < 100 {
            ids.push(id);
        }
    }
    let cad = CadDocument::from_document(doc);
    let pairs: Vec<_> = ids.iter().map(|&id| (id, comb())).collect();
    let tol = Tolerance::DEFAULT;
    let offset = Vector2::new(2.0, 3.0).expect("finite");
    let polygons = selection_polygons(pairs.clone(), tol).expect("valid");
    let shapes = cad
        .stretch_preview(&ids, offset, &[], &polygons)
        .expect("preview");
    assert_eq!(shapes.len(), 100);

    let mut group = c.benchmark_group("STRETCH crossing polygon");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5));
    group.bench_function("validate per pair (402 corners x 100)", |b| {
        b.iter(|| {
            let each: Vec<_> = pairs
                .iter()
                .map(|(id, points)| (*id, SelectionPolygon::new(points.clone(), tol).unwrap()))
                .collect();
            black_box(each)
        });
    });
    group.bench_function("validate shared (402 corners x 100)", |b| {
        b.iter(|| black_box(selection_polygons(pairs.clone(), tol).unwrap()));
    });
    group.bench_function("preview, polygons validated", |b| {
        b.iter(|| black_box(cad.stretch_preview(&ids, offset, &[], &polygons).unwrap()));
    });
    group.finish();
}

criterion_group!(benches, queries, quick_edits, stretch_polygon);
criterion_main!(benches);
