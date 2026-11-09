//! Tests of the GPU renderer (a submodule of `gpu`): each scene drawn by the
//! CPU rasterizer, the floor path, and by the GPU, which must draw the same
//! thing (ADR 0013 D5.8).
//!
//! They need a graphics adapter. Without one they say so and pass, unless
//! `ARQ_GPU_TESTS=require` is set, as on the reference machine, where a
//! missing adapter fails them. `ARQ_GPU_BACKEND` picks the backend (`vulkan`,
//! `opengl`, `software`, ...); by default a hardware adapter, else the first
//! software one.
//!
//! Antialiasing differs between tiny-skia and the GPU, so images are compared
//! by what they light: every pixel one draws must lie within one pixel of a
//! pixel the other draws, and full colors must be the same.

use arq_core::{CadDocument, Color, DrawItem, Shape};
use arq_geometry::{Bounds2, Point2};

use super::*;
use crate::raster::render;

const DARK: Style = Style {
    background: [33, 40, 48],
};
const LIGHT: Style = Style {
    background: [250, 250, 250],
};
const WHITE: [u8; 3] = [255, 255, 255];

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// A renderer, or `None` with a note when this machine has no adapter.
fn renderer() -> Option<GpuRenderer> {
    let wanted = std::env::var("ARQ_GPU_BACKEND")
        .ok()
        .filter(|v| !v.is_empty());
    let candidates: Vec<GpuBackend> = match wanted.as_deref() {
        Some(name) => vec![GpuBackend::parse(name).expect("ARQ_GPU_BACKEND")],
        None => vec![
            GpuBackend::Dx12,
            GpuBackend::Metal,
            GpuBackend::Vulkan,
            GpuBackend::OpenGl,
            GpuBackend::Software,
        ],
    };
    let mut errors = Vec::new();
    let mut software = None;
    for backend in candidates {
        match GpuRenderer::new(backend) {
            Ok(r) if r.adapter().device_type != "Cpu" => return Some(r),
            Ok(r) => software = software.or(Some(r)),
            Err(e) => errors.push(format!("{}: {e}", backend.name())),
        }
    }
    if software.is_some() {
        return software;
    }
    let required = std::env::var("ARQ_GPU_TESTS").is_ok_and(|v| v == "require");
    assert!(
        !required,
        "ARQ_GPU_TESTS=require but no adapter: {errors:?}"
    );
    eprintln!("no graphics adapter, GPU renderer not checked: {errors:?}");
    None
}

fn pixel(img: &Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
}

fn lit(img: &Image, style: Style) -> Vec<bool> {
    img.pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| [px[0], px[1], px[2]] != style.background)
        .collect()
}

/// Whether `mask` lights a pixel within one of `(x, y)`.
fn near(mask: &[bool], width: u32, height: u32, x: u32, y: u32) -> bool {
    let (x, y) = (i64::from(x), i64::from(y));
    (-1..=1).any(|dy| {
        (-1..=1).any(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            nx >= 0
                && ny >= 0
                && nx < i64::from(width)
                && ny < i64::from(height)
                && mask[(ny * i64::from(width) + nx) as usize]
        })
    })
}

/// The GPU draws what the CPU draws: same size, the same pixels lit to
/// within one pixel either way, and each of the shapes' `inks` in both.
/// Returns how many pixels the CPU lit, so a case that draws nothing shows.
fn assert_same(cpu: &Image, gpu: &Image, style: Style, inks: &[[u8; 3]], case: &str) -> usize {
    assert_eq!(
        (gpu.width, gpu.height, gpu.pixels.len()),
        (cpu.width, cpu.height, cpu.pixels.len()),
        "{case}: size"
    );
    assert!(
        gpu.pixels.as_chunks::<4>().0.iter().all(|px| px[3] == 255),
        "{case}: opaque"
    );
    let (w, h) = (cpu.width, cpu.height);
    let (cpu_lit, gpu_lit) = (lit(cpu, style), lit(gpu, style));
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            assert!(
                !gpu_lit[i] || near(&cpu_lit, w, h, x, y),
                "{case}: the GPU lit ({x}, {y}), far from what the CPU drew"
            );
            assert!(
                !cpu_lit[i] || near(&gpu_lit, w, h, x, y),
                "{case}: the GPU left ({x}, {y}) and all around it empty"
            );
        }
    }
    let has = |img: &Image, ink: [u8; 3]| {
        img.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| [px[0], px[1], px[2]] == ink)
    };
    for &ink in inks {
        assert!(has(cpu, ink), "{case}: the CPU never draws {ink:?}");
        assert!(has(gpu, ink), "{case}: the GPU never draws {ink:?}");
    }
    cpu_lit.iter().filter(|&&l| l).count()
}

fn both(gpu: &mut GpuRenderer, scene: &Scene, camera: &Camera, style: Style) -> (Image, Image) {
    let cpu = render(scene, camera, style);
    let drawn = gpu.render(scene, camera, style).expect("the GPU draws");
    (cpu, drawn)
}

#[test]
fn the_gpu_draws_what_the_cpu_draws() {
    let Some(mut gpu) = renderer() else {
        return;
    };
    eprintln!(
        "GPU renderer on {:?}, {} samples",
        gpu.adapter(),
        gpu.samples()
    );
    let camera = Camera::new(100, 100);

    // A line on row 50, hand-checked as in the rasterizer's tests.
    let mut cad = CadDocument::new();
    cad.add_line(p(-30.0, -0.5), p(30.0, -0.5)).unwrap();
    let scene = Scene::build(cad.document());
    let (cpu, img) = both(&mut gpu, &scene, &camera, DARK);
    assert_eq!(pixel(&img, 50, 50), [255, 255, 255], "ACI 7 white on dark");
    assert_eq!(pixel(&img, 50, 45), DARK.background, "rows away stay empty");
    assert_eq!(pixel(&img, 10, 50), DARK.background, "beyond its end");
    assert!(assert_same(&cpu, &img, DARK, &[WHITE], "a line") > 0);

    // Colors on a light background.
    let mut cad = CadDocument::new();
    let (red, _) = cad.add_line(p(-30.0, 10.5), p(30.0, 10.5)).unwrap();
    cad.set_entity_color(&[red], Color::Index(1)).unwrap();
    let (rgb, _) = cad.add_line(p(-30.0, -10.5), p(30.0, -10.5)).unwrap();
    let green = Color::Rgb {
        r: 10,
        g: 200,
        b: 30,
    };
    cad.set_entity_color(&[rgb], green).unwrap();
    cad.add_line(p(-30.0, -20.5), p(30.0, -20.5)).unwrap();
    let scene = Scene::build(cad.document());
    let (cpu, img) = both(&mut gpu, &scene, &camera, LIGHT);
    assert_eq!(pixel(&img, 50, 39), [255, 0, 0]);
    assert_eq!(pixel(&img, 50, 60), [10, 200, 30]);
    assert_eq!(pixel(&img, 50, 70), [0, 0, 0], "ACI 7 black on light");
    assert!(
        assert_same(
            &cpu,
            &img,
            LIGHT,
            &[[255, 0, 0], [10, 200, 30], [0, 0, 0]],
            "colors"
        ) > 0
    );

    // Circles and arcs, large and down to a dot, and a diagonal.
    let mut cad = CadDocument::new();
    cad.add_circle(p(0.0, 0.0), 20.5).unwrap();
    cad.add_circle(p(-30.0, 30.0), 3.0).unwrap();
    cad.add_circle(p(30.0, 30.0), 0.2).unwrap();
    cad.add_arc(p(0.0, 0.0), 40.0, 0.3, 2.5).unwrap();
    cad.add_line(p(-45.0, -45.0), p(45.0, -10.0)).unwrap();
    let scene = Scene::build(cad.document());
    let (cpu, img) = both(&mut gpu, &scene, &camera, DARK);
    assert_eq!(pixel(&img, 50, 50), DARK.background, "a circle is hollow");
    assert!(assert_same(&cpu, &img, DARK, &[WHITE], "curves") > 0);

    // Points, and an entity not drawn yet outlined dashed, on pixel middles
    // so both draw them in full.
    let items = vec![
        DrawItem {
            shape: Shape::Point(p(-20.5, 20.5)),
            color: Color::Index(2),
        },
        DrawItem {
            shape: Shape::Placeholder(Bounds2::new(p(-10.5, -30.5), p(40.5, 10.5))),
            color: Color::Index(4),
        },
    ];
    let scene = Scene::from_items(items);
    let (cpu, img) = both(&mut gpu, &scene, &camera, DARK);
    assert!(
        assert_same(
            &cpu,
            &img,
            DARK,
            &[[255, 255, 0], [0, 255, 255]],
            "points and dashes"
        ) > 0
    );

    // Survey coordinates, and a line zoomed in a million times.
    let mut cad = CadDocument::new();
    let (x0, y0) = (500_000.0, 9_977_000.0);
    cad.add_line(p(x0 - 30.0, y0 - 0.5), p(x0 + 30.0, y0 - 0.5))
        .unwrap();
    let mut survey = camera;
    survey.fit(Bounds2::new(
        p(x0 - 45.0, y0 - 45.0),
        p(x0 + 45.0, y0 + 45.0),
    ));
    let (cpu, img) = both(&mut gpu, &Scene::build(cad.document()), &survey, DARK);
    assert_eq!(
        pixel(&img, 50, 50),
        [255, 255, 255],
        "row 50, as at the origin"
    );
    assert!(assert_same(&cpu, &img, DARK, &[WHITE], "survey coordinates") > 0);
    let mut cad = CadDocument::new();
    cad.add_line(p(-1000.0, 0.0000005), p(1000.0, 0.0000005))
        .unwrap();
    let mut close = camera;
    close.zoom_at(50.0, 50.0, 1e6);
    let (cpu, img) = both(&mut gpu, &Scene::build(cad.document()), &close, DARK);
    assert_eq!(pixel(&img, 0, 49), [255, 255, 255]);
    assert_eq!(pixel(&img, 99, 49), [255, 255, 255]);
    assert!(assert_same(&cpu, &img, DARK, &[WHITE], "a million times closer") > 0);

    // A busy drawing at another size, which renews the GPU's images.
    let mut cad = CadDocument::new();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 * 200.0 - 100.0
    };
    for _ in 0..400 {
        let (x, y) = (next(), next());
        cad.add_line(p(x, y), p(x + next() / 10.0, y + next() / 10.0))
            .unwrap();
    }
    let mut wide = Camera::new(320, 180);
    let scene = Scene::build(cad.document());
    wide.fit(scene.bounds().unwrap());
    let (cpu, img) = both(&mut gpu, &scene, &wide, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "400 lines at 320x180") > 1000);

    // Nothing to draw: the background alone.
    let img = gpu.render(&Scene::default(), &camera, DARK).expect("empty");
    assert!(lit(&img, DARK).iter().all(|&l| !l), "an empty scene");
}
