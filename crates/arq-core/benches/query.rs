//! Queries, snapshots and transactions on a 100,000-entity drawing (contract §12, ADR 0011 D3.8).
//!
//! The baseline for the phase 7 spatial index: `cargo bench -p arq-core`.
//! A reproducible measurement target, not a promise.

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
    let cursor = p(0.5 * extent + 0.1, 0.5 * extent + 0.1);
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
                cad.snap(black_box(cursor), 1.0, &modes, None)
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

criterion_group!(benches, queries);
criterion_main!(benches);
