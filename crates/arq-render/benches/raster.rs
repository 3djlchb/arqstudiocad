//! Rasterizing a 100,000-entity drawing at 1920 x 1080 (ADR 0013, D5.3).
//!
//! `cargo bench -p arq-render`. A measurement target for the floor machine
//! (ADR 0013, D5.8), not a promise.

use std::hint::black_box;

use arq_core::{Document, PlanGeometry};
use arq_geometry::{Arc, Circle, Geometry2, Line, Point2};
use arq_render::{Camera, Scene, Style, render};
use criterion::{Criterion, criterion_group, criterion_main};

const SIDE: usize = 317; // 317² ≈ 100,000 cells, one entity each
const STEP: f64 = 10.0;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).expect("finite")
}

fn drawing() -> Document {
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
    doc
}

fn rasterize(c: &mut Criterion) {
    let doc = drawing();
    let scene = Scene::build(&doc);
    let mut all = Camera::new(1920, 1080);
    all.fit(scene.bounds().expect("not empty"));
    let mut detail = all;
    detail.zoom_at(960.0, 540.0, 10.0); // about 1% of the drawing
    let style = Style::default();

    let mut group = c.benchmark_group("100k entities at 1920x1080");
    group.sample_size(10);
    group.bench_function("build scene from the document", |b| {
        b.iter(|| black_box(Scene::build(&doc)))
    });
    group.bench_function("render all", |b| {
        b.iter(|| black_box(render(&scene, &all, style)))
    });
    group.bench_function("render 1% (zoomed in)", |b| {
        b.iter(|| black_box(render(&scene, &detail, style)))
    });
    group.finish();
}

criterion_group!(benches, rasterize);
criterion_main!(benches);
