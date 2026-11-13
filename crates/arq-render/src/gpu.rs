//! The GPU renderer (ADR 0007, ADR 0034): a scene and a camera drawn with
//! wgpu, either straight into a window or offscreen into the CPU
//! rasterizer's [`Image`].
//!
//! The drawing is kept on the GPU ([`Retained`]): panning and zooming
//! change only a uniform, and the geometry is built and uploaded again only
//! when the scene changes, the zoom leaves its band or the view leaves its
//! region. Lines are one pixel wide, and what can be shorter than a pixel
//! is drawn as a pixel too, as the CPU shows it, within wgpu's downlevel
//! limits (what OpenGL ES 3.0 offers); 4× multisampled on request, where a
//! hardware adapter offers it.
//! Dashes are cut in the fragment shader along each segment from its start,
//! as the rasterizer cuts them.
//!
//! The window is the viewport's (ADR 0034 D1), its surface made from Qt's
//! native handles at the Python boundary; the offscreen image lets the
//! GPU be checked against the CPU rasterizer, the floor path, on the same
//! input (ADR 0013 D5.8). Errors come back as errors, never panics, so the
//! caller can draw on the CPU instead (ADR 0034 D5).

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use crate::camera::Camera;
use crate::gpu_overlay::OverlayPainter;
use crate::gpu_probe::{GpuBackend, GpuProbeResult, offscreen_adapter};
use crate::overlay::Overlay;
use crate::raster::{DASH_PX, Image, Style};
use crate::retained::{Retained, Vertex};
use crate::scene::Scene;

/// The offscreen image: 8 bits per channel, no sRGB conversion, so colors
/// come back byte for byte.
const OFFSCREEN: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// How long a frame may take on the GPU before the renderer gives up.
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
/// Bytes per vertex: position, offset, along and color.
const VERTEX_BYTES: u64 = 24;

const SHADER: &str = r"
struct View {
    // The camera's center relative to the geometry's origin, in world units.
    center: vec2<f32>,
    // The target's size in pixels.
    size: vec2<f32>,
    // Pixels per world unit.
    scale: f32,
    // Dashes: on and on plus off, in pixels.
    dash_on: f32,
    dash_period: f32,
    unused: f32,
};
@group(0) @binding(0) var<uniform> view: View;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Pixels from the segment's start; negative when not dashed.
    @location(1) along: f32,
};

@vertex
fn vs_main(
    @location(0) at: vec2<f32>,
    @location(1) offset: vec2<f32>,
    @location(2) along: f32,
    @location(3) color: vec4<f32>,
) -> Out {
    return place(at, offset, along, color);
}

fn place(at: vec2<f32>, offset: vec2<f32>, along: f32, color: vec4<f32>) -> Out {
    let d = (at - view.center) * view.scale;
    // Screen pixels: origin top-left, y down.
    let px = vec2<f32>(
        d.x + view.size.x * 0.5 + offset.x,
        view.size.y * 0.5 - d.y + offset.y,
    );
    var out: Out;
    out.position = vec4<f32>(
        px.x / view.size.x * 2.0 - 1.0,
        1.0 - px.y / view.size.y * 2.0,
        0.0,
        1.0,
    );
    out.color = color;
    out.along = along * view.scale;
    return out;
}

// A dot, only where it falls inside the view: a pixel just outside would
// still bleed into the edge, where the CPU draws nothing.
@vertex
fn vs_end(
    @location(0) at: vec2<f32>,
    @location(1) offset: vec2<f32>,
    @location(2) along: f32,
    @location(3) color: vec4<f32>,
) -> Out {
    var out = place(at, offset, along, color);
    let px = (out.position.xy * vec2<f32>(0.5, -0.5) + 0.5) * view.size;
    if (px.x < 0.0 || px.y < 0.0 || px.x >= view.size.x || px.y >= view.size.y) {
        out.position = vec4<f32>(4.0, 4.0, 0.0, 1.0);
    }
    return out;
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    if (in.along >= 0.0 && in.along % view.dash_period >= view.dash_on) {
        discard;
    }
    return in.color;
}
";

/// A scene drawn on one GPU adapter, into a window or offscreen.
pub struct GpuRenderer {
    adapter: GpuProbeResult,
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: Vec<(
        wgpu::TextureFormat,
        wgpu::PrimitiveTopology,
        wgpu::RenderPipeline,
    )>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    samples: u32,
    max_buffer: u64,
    retained: Option<Retained>,
    vertices: Option<wgpu::Buffer>,
    triangle_count: u32,
    line_count: u32,
    dot_count: u32,
    end_count: u32,
    builds: u64,
    offscreen: Option<Offscreen>,
    window: Option<Window>,
    errors: Arc<Mutex<Option<String>>>,
    overlay: OverlayPainter,
    /// Keeps the surface's instance alive.
    _instance: Option<wgpu::Instance>,
}

/// The fills as triangles (ADR 0038), the segments as lines, and what can
/// be shorter than a pixel as pixels.
struct Pipelines {
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    dots: wgpu::RenderPipeline,
}

/// The offscreen textures and the readback buffer, for one size.
struct Offscreen {
    width: u32,
    height: u32,
    /// Bytes per row in the readback buffer, padded as wgpu requires.
    row_bytes: u32,
    resolved: wgpu::Texture,
    multisampled: Option<wgpu::Texture>,
    readback: wgpu::Buffer,
}

/// The window's surface and its multisampled image, if any.
struct Window {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    multisampled: Option<wgpu::Texture>,
}

impl GpuRenderer {
    /// An offscreen renderer on `backend`'s adapter, the one its probe
    /// checks, 4× multisampled when `antialias` asks and a hardware adapter
    /// offers it. Run the probe in a child process first (ADR 0007): a
    /// driver can crash where an error cannot be caught.
    pub fn new(backend: GpuBackend, antialias: bool) -> Result<Self, String> {
        pollster::block_on(async {
            let adapter = offscreen_adapter(backend).await?;
            Self::create(backend, adapter, None, antialias).await
        })
    }

    /// A renderer presenting to a window's `surface`, made by `instance`
    /// on `backend`, `size` in pixels, with `adapter`: the card at that
    /// index of the backend's list, checked by its name (a backend may list
    /// one card twice, Vulkan the UHD 630, and one of the two crash), or
    /// the one `backend` prefers (the discrete card where there are two)
    /// when it is None. Making the surface from the window's native handles
    /// is the caller's: the one `unsafe` step, kept at the Python boundary
    /// (ADR 0034 D1).
    pub fn for_surface(
        backend: GpuBackend,
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        size: (u32, u32),
        antialias: bool,
        adapter: Option<(usize, &str)>,
    ) -> Result<Self, String> {
        let (width, height) = size;
        pollster::block_on(async {
            let adapter = match adapter {
                Some((index, name)) => {
                    let found = instance
                        .enumerate_adapters(backend.wgpu_backends())
                        .await
                        .into_iter()
                        .nth(index)
                        .filter(|a| a.get_info().name == name)
                        .ok_or_else(|| {
                            format!("{name} is no longer card {index} of {}", backend.name())
                        })?;
                    if !found.is_surface_supported(&surface) {
                        return Err(format!("{name} cannot draw in this window"));
                    }
                    found
                }
                None => instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        force_fallback_adapter: false,
                        compatible_surface: Some(&surface),
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| e.to_string())?,
            };
            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| !f.is_srgb())
                .or_else(|| caps.formats.first().copied())
                .ok_or("the window offers no surface format")?;
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.max(1),
                height: height.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: caps
                    .alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Auto),
                view_formats: vec![],
                color_space: wgpu::SurfaceColorSpace::Auto,
            };
            let mut renderer = Self::create(backend, adapter, Some(instance), antialias).await?;
            surface.configure(&renderer.device, &config);
            renderer.window = Some(Window {
                surface,
                config,
                multisampled: None,
            });
            renderer.check()?;
            Ok(renderer)
        })
    }

    async fn create(
        backend: GpuBackend,
        adapter: wgpu::Adapter,
        instance: Option<wgpu::Instance>,
        antialias: bool,
    ) -> Result<Self, String> {
        let info = adapter.get_info();
        // The downlevel rule (ADR 0007): what an OpenGL ES 3.0 device offers.
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let max_buffer = limits.max_buffer_size;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ArqStudioCAD renderer"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let errors: Arc<Mutex<Option<String>>> = Arc::default();
        let sink = Arc::clone(&errors);
        device.on_uncaptured_error(Arc::new(move |error| {
            let mut slot = sink
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            slot.get_or_insert_with(|| error.to_string());
        }));
        // Multisampling when asked, on hardware only: on a CPU adapter it
        // costs much and gains nothing, and lavapipe drops pixels of
        // multisampled lines (a 5-pixel line drawn as 2, measured on the
        // reference machine). On the GT 730 it doubles a frame's time.
        let samples = if antialias
            && info.device_type != wgpu::DeviceType::Cpu
            && adapter
                .get_texture_format_features(OFFSCREEN)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ArqStudioCAD renderer shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ArqStudioCAD renderer view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ArqStudioCAD renderer layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ArqStudioCAD renderer view"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ArqStudioCAD renderer view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let overlay = OverlayPainter::new(&device, &layout, samples);
        let renderer = Self {
            adapter: GpuProbeResult {
                backend: backend.name().to_owned(),
                driver: crate::gpu_probe::driver_of(&info),
                adapter: info.name,
                device_type: format!("{:?}", info.device_type),
            },
            device,
            queue,
            shader,
            pipeline_layout,
            pipelines: Vec::new(),
            uniform,
            bind_group,
            samples,
            max_buffer,
            retained: None,
            vertices: None,
            triangle_count: 0,
            line_count: 0,
            dot_count: 0,
            end_count: 0,
            builds: 0,
            offscreen: None,
            window: None,
            errors,
            overlay,
            _instance: instance,
        };
        renderer.check()?;
        Ok(renderer)
    }

    /// The backend and adapter it draws on.
    pub fn adapter(&self) -> &GpuProbeResult {
        &self.adapter
    }

    /// Samples per pixel: 4 when antialiasing was asked and a hardware
    /// adapter offers it, else 1.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// How many times the drawing was built and uploaded: once, then only
    /// when the scene, the background, the zoom band or the region changed.
    pub fn builds(&self) -> u64 {
        self.builds
    }

    /// Draws `scene` as `camera` sees it into the window, sized to the
    /// camera. A frame the window cannot take now (hidden, being resized)
    /// is skipped. An error means the GPU cannot draw; the caller draws on
    /// the CPU instead.
    pub fn present(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: Style,
        overlay: &Overlay,
    ) -> Result<(), String> {
        let (width, height) = camera.size();
        self.prepare(scene, camera, style)?;
        let Some(window) = self.window.as_mut() else {
            return Err("this renderer has no window".to_owned());
        };
        if (window.config.width, window.config.height) != (width, height) {
            window.config.width = width;
            window.config.height = height;
            window.surface.configure(&self.device, &window.config);
            window.multisampled = None;
        }
        let frame = match window.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                window.surface.configure(&self.device, &window.config);
                return Ok(());
            }
            other => return Err(format!("the window's surface: {other:?}")),
        };
        let format = window.config.format;
        if self.samples > 1 && window.multisampled.is_none() {
            window.multisampled = Some(texture(
                &self.device,
                width,
                height,
                self.samples,
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ));
        }
        let target = frame.texture.create_view(&Default::default());
        let multisampled = window
            .multisampled
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        let pipelines = self.pipelines_for(format);
        self.overlay
            .prepare(&self.device, &self.queue, overlay, format);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.pass(
            &mut encoder,
            &target,
            multisampled.as_ref(),
            &pipelines,
            style,
            format,
        );
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.check()
    }

    /// Paints `scene` as `camera` sees it into an image, like
    /// [`crate::render`], with `overlay` over it: the GPU's drawing,
    /// checked against the CPU's.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: Style,
        overlay: &Overlay,
    ) -> Result<Image, String> {
        let (width, height) = camera.size();
        self.prepare(scene, camera, style)?;
        if !self
            .offscreen
            .as_ref()
            .is_some_and(|t| (t.width, t.height) == (width, height))
        {
            self.offscreen = Some(self.new_offscreen(width, height));
        }
        let pipelines = self.pipelines_for(OFFSCREEN);
        self.overlay
            .prepare(&self.device, &self.queue, overlay, OFFSCREEN);
        let Some(target) = &self.offscreen else {
            unreachable!("made just above");
        };
        let resolved = target.resolved.create_view(&Default::default());
        let multisampled = target
            .multisampled
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (view, resolve) = match &multisampled {
            Some(m) => (m, Some(&resolved)),
            None => (&resolved, None),
        };
        self.pass_into(&mut encoder, view, resolve, &pipelines, style, OFFSCREEN);
        encoder.copy_texture_to_buffer(
            target.resolved.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(target.row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        target
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(FRAME_TIMEOUT),
            })
            .map_err(|error| error.to_string())?;
        receiver
            .recv_timeout(FRAME_TIMEOUT)
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let mapped = target
            .readback
            .slice(..)
            .get_mapped_range()
            .map_err(|error| error.to_string())?;
        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        for y in 0..height as usize {
            let start = y * target.row_bytes as usize;
            pixels.extend_from_slice(&mapped[start..start + row]);
        }
        drop(mapped);
        target.readback.unmap();
        self.check()?;
        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    /// Builds and uploads the drawing if what is kept no longer serves,
    /// then sets the camera.
    fn prepare(&mut self, scene: &Scene, camera: &Camera, style: Style) -> Result<(), String> {
        if !self
            .retained
            .as_ref()
            .is_some_and(|r| r.serves(scene, camera, style))
        {
            let retained =
                Retained::build(scene, camera, style).ok_or("the camera shows nothing finite")?;
            self.upload(
                &retained.triangles,
                &retained.vertices,
                &retained.dots,
                &retained.ends,
            )?;
            self.retained = Some(retained);
            self.builds += 1;
        }
        let Some(retained) = &self.retained else {
            unreachable!("built just above");
        };
        let (cx, cy) = retained.center(camera);
        let (width, height) = camera.size();
        let [on, off] = DASH_PX;
        let view = [
            cx as f32,
            cy as f32,
            width as f32,
            height as f32,
            camera.scale() as f32,
            on,
            on + off,
            0.0,
        ];
        let bytes: Vec<u8> = view.iter().flat_map(|v| v.to_ne_bytes()).collect();
        self.queue.write_buffer(&self.uniform, 0, &bytes);
        Ok(())
    }

    /// Uploads the triangles, then the lines, the dots and the ends.
    fn upload(
        &mut self,
        triangles: &[Vertex],
        lines: &[Vertex],
        dots: &[Vertex],
        ends: &[Vertex],
    ) -> Result<(), String> {
        let count = triangles.len() + lines.len() + dots.len() + ends.len();
        let bytes = count as u64 * VERTEX_BYTES;
        if bytes > self.max_buffer {
            return Err(format!(
                "{} segments exceed the GPU's {}-byte buffers",
                lines.len() / 2,
                self.max_buffer
            ));
        }
        let needed = bytes.max(VERTEX_BYTES * 2);
        if !self.vertices.as_ref().is_some_and(|b| b.size() >= needed) {
            let size = (needed + needed / 2).min(self.max_buffer).max(needed);
            self.vertices = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ArqStudioCAD renderer segments"),
                size: size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let mut data = Vec::with_capacity(bytes as usize);
        for v in triangles.iter().chain(lines).chain(dots).chain(ends) {
            for f in [v.at[0], v.at[1], v.offset[0], v.offset[1], v.along] {
                data.extend_from_slice(&f.to_ne_bytes());
            }
            data.extend_from_slice(&v.rgba);
        }
        if let (Some(buffer), false) = (&self.vertices, data.is_empty()) {
            self.queue.write_buffer(buffer, 0, &data);
        }
        self.triangle_count = triangles.len() as u32;
        self.line_count = lines.len() as u32;
        self.dot_count = dots.len() as u32;
        self.end_count = ends.len() as u32;
        Ok(())
    }

    /// The pass into a window's frame, multisampled or not.
    fn pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        multisampled: Option<&wgpu::TextureView>,
        pipelines: &Pipelines,
        style: Style,
        format: wgpu::TextureFormat,
    ) {
        match multisampled {
            Some(m) => self.pass_into(encoder, m, Some(frame), pipelines, style, format),
            None => self.pass_into(encoder, frame, None, pipelines, style, format),
        }
    }

    fn pass_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
        pipelines: &Pipelines,
        style: Style,
        format: wgpu::TextureFormat,
    ) {
        let [r, g, b] = style.background.map(|c| f64::from(c) / 255.0);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ArqStudioCAD renderer lines"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        // In the buffer: the triangles, the lines, the dots, then the ends.
        let fills = self.triangle_count;
        let total = fills + self.line_count + self.dot_count + self.end_count;
        if let (Some(buffer), true) = (&self.vertices, total > 0) {
            pass.set_vertex_buffer(0, buffer.slice(..u64::from(total) * VERTEX_BYTES));
            // The fills first, under every line, as the CPU paints them.
            pass.set_pipeline(&pipelines.triangles);
            pass.draw(0..fills, 0..1);
            pass.set_pipeline(&pipelines.lines);
            pass.draw(fills..fills + self.line_count, 0..1);
            // What can be shorter than a pixel, as pixels: a line that short
            // makes no fragment, and the CPU still shows it as a dot.
            // Without multisampling a line also leaves out its last pixel
            // (the diamond-exit rule), which the CPU draws: then the ends of
            // lines and arcs are drawn as pixels too.
            let last = if self.samples == 1 {
                total
            } else {
                fills + self.line_count + self.dot_count
            };
            pass.set_pipeline(&pipelines.dots);
            pass.draw(fills + self.line_count..last, 0..1);
        }
        self.overlay.draw(&mut pass, format);
    }

    /// The pipelines for a target format.
    fn pipelines_for(&mut self, format: wgpu::TextureFormat) -> Pipelines {
        Pipelines {
            triangles: self.pipeline(format, wgpu::PrimitiveTopology::TriangleList),
            lines: self.pipeline(format, wgpu::PrimitiveTopology::LineList),
            dots: self.pipeline(format, wgpu::PrimitiveTopology::PointList),
        }
    }

    /// A pipeline for a target format and topology, made once.
    fn pipeline(
        &mut self,
        format: wgpu::TextureFormat,
        topology: wgpu::PrimitiveTopology,
    ) -> wgpu::RenderPipeline {
        if let Some((_, _, p)) = self
            .pipelines
            .iter()
            .find(|(f, t, _)| *f == format && *t == topology)
        {
            return p.clone();
        }
        let attribute = |format, offset, shader_location| wgpu::VertexAttribute {
            format,
            offset,
            shader_location,
        };
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ArqStudioCAD renderer lines"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(match topology {
                        wgpu::PrimitiveTopology::PointList => "vs_end",
                        _ => "vs_main",
                    }),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: VERTEX_BYTES,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            attribute(wgpu::VertexFormat::Float32x2, 0, 0),
                            attribute(wgpu::VertexFormat::Float32x2, 8, 1),
                            attribute(wgpu::VertexFormat::Float32, 16, 2),
                            attribute(wgpu::VertexFormat::Unorm8x4, 20, 3),
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: self.samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        self.pipelines.push((format, topology, pipeline.clone()));
        pipeline
    }

    fn new_offscreen(&self, width: u32, height: u32) -> Offscreen {
        let resolved = texture(
            &self.device,
            width,
            height,
            1,
            OFFSCREEN,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let multisampled = (self.samples > 1).then(|| {
            texture(
                &self.device,
                width,
                height,
                self.samples,
                OFFSCREEN,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let row_bytes = (width * 4).div_ceil(align) * align;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ArqStudioCAD renderer readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Offscreen {
            width,
            height,
            row_bytes,
            resolved,
            multisampled,
            readback,
        }
    }

    /// The first error wgpu reported since the last check, if any.
    fn check(&self) -> Result<(), String> {
        let mut slot = self
            .errors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match slot.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    samples: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ArqStudioCAD renderer image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
