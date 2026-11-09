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
//! by what they draw: where one draws a pixel with some contrast, the other
//! must draw something, however faint, within one pixel of it; and full
//! colors must be the same.

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
/// Below this difference from the background in every channel, a pixel is
/// an antialiasing ghost.
const GHOST: u8 = 24;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// A renderer, or `None` with a note when this machine has no adapter.
fn renderer(antialias: bool) -> Option<GpuRenderer> {
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
        // Only a missing adapter skips: where one exists, the renderer must
        // build on it, or the test fails.
        if let Err(e) = pollster::block_on(crate::gpu_probe::offscreen_adapter(backend)) {
            errors.push(format!("{}: {e}", backend.name()));
            continue;
        }
        let r = GpuRenderer::new(backend, antialias)
            .unwrap_or_else(|e| panic!("{}: an adapter, but no renderer: {e}", backend.name()));
        if r.adapter().device_type != "Cpu" {
            return Some(r);
        }
        software = software.or(Some(r));
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

/// Pixels touched at all, however faintly.
fn drew(img: &Image, style: Style) -> Vec<bool> {
    img.pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| [px[0], px[1], px[2]] != style.background)
        .collect()
}

/// Pixels drawn with some contrast. Antialiasing spreads a line's faint
/// edges differently in tiny-skia and on the GPU.
fn lit(img: &Image, style: Style) -> Vec<bool> {
    img.pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| (0..3).any(|c| px[c].abs_diff(style.background[c]) >= GHOST))
        .collect()
}

/// Whether `mask` lights a pixel within one of `(x, y)`.
/// Beyond the image's edge nothing can be seen, so a neighbor there counts:
/// a line a tenth of a pixel outside the view lights the edge row on the
/// CPU, whose antialiasing spreads half a pixel, and the row beyond on the
/// GPU.
fn near(mask: &[bool], width: u32, height: u32, x: u32, y: u32) -> bool {
    let (x, y) = (i64::from(x), i64::from(y));
    (-1..=1).any(|dy| {
        (-1..=1).any(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            let outside = nx < 0 || ny < 0 || nx >= i64::from(width) || ny >= i64::from(height);
            outside || mask[(ny * i64::from(width) + nx) as usize]
        })
    })
}

/// The GPU draws what the CPU draws: same size, what either draws clearly
/// drawn by the other within one pixel, and each of the shapes' `inks` in
/// both.
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
    let (cpu_drew, gpu_drew) = (drew(cpu, style), drew(gpu, style));
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            assert!(
                !gpu_lit[i] || near(&cpu_drew, w, h, x, y),
                "{case}: the GPU lit ({x}, {y}) {:?}, far from what the CPU drew",
                pixel(gpu, x, y)
            );
            assert!(
                !cpu_lit[i] || near(&gpu_drew, w, h, x, y),
                "{case}: the GPU left ({x}, {y}) and all around it empty, \
                 where the CPU drew {:?}",
                pixel(cpu, x, y)
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
    for antialias in [false, true] {
        let Some(mut gpu) = renderer(antialias) else {
            return;
        };
        draws_what_the_cpu_draws(&mut gpu);
    }
}

fn draws_what_the_cpu_draws(gpu: &mut GpuRenderer) {
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
    let (cpu, img) = both(gpu, &scene, &camera, DARK);
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
    let (cpu, img) = both(gpu, &scene, &camera, LIGHT);
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
    let (cpu, img) = both(gpu, &scene, &camera, DARK);
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
    let (cpu, img) = both(gpu, &scene, &camera, DARK);
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
    let (cpu, img) = both(gpu, &Scene::build(cad.document()), &survey, DARK);
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
    let (cpu, img) = both(gpu, &Scene::build(cad.document()), &close, DARK);
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
    let (cpu, img) = both(gpu, &scene, &wide, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "400 lines at 320x180") > 1000);

    // A long diagonal seen a million times closer: kept relative to its
    // region, it lands where the CPU puts it, not off by its endpoints'
    // rounding.
    let mut cad = CadDocument::new();
    // Through (0, -0.00003): inside the view, which spans ±0.00005.
    cad.add_line(p(-1000.0, -1000.00003), p(1000.0, 999.99997))
        .unwrap();
    let mut close = camera;
    close.zoom_at(50.0, 50.0, 1e6);
    let (cpu, img) = both(gpu, &Scene::build(cad.document()), &close, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "a diagonal 10^6 closer") > 50);

    // Kept on the GPU: panning within the region and zooming within the
    // band draw from what was built; leaving the band builds again. Each
    // frame matches the CPU.
    let scene = Scene::build(cad_with_lines(300).document());
    let mut cam = Camera::new(160, 120);
    cam.fit(scene.bounds().unwrap());
    let (cpu, img) = both(gpu, &scene, &cam, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "kept: first") > 200);
    let built = gpu.builds();
    cam.pan(37.0, -21.0);
    cam.zoom_at(80.0, 60.0, 1.6);
    let (cpu, img) = both(gpu, &scene, &cam, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "kept: panned, zoomed") > 100);
    assert_eq!(gpu.builds(), built, "nothing built again");
    cam.zoom_at(80.0, 60.0, 1.6);
    let (cpu, img) = both(gpu, &scene, &cam, DARK);
    assert!(assert_same(&cpu, &img, DARK, &[], "kept: past the band") > 50);
    assert_eq!(gpu.builds(), built + 1, "built again past the band");

    // Nothing to draw: the background alone.
    let img = gpu.render(&Scene::default(), &camera, DARK).expect("empty");
    assert!(lit(&img, DARK).iter().all(|&l| !l), "an empty scene");
}

/// `count` random short lines over ±100.
fn cad_with_lines(count: usize) -> CadDocument {
    let mut cad = CadDocument::new();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 * 200.0 - 100.0
    };
    for _ in 0..count {
        let (x, y) = (next(), next());
        cad.add_line(p(x, y), p(x + next() / 10.0, y + next() / 10.0))
            .unwrap();
    }
    cad
}
