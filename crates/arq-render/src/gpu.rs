//! The GPU renderer: the same scene and camera as the CPU rasterizer, drawn
//! offscreen with wgpu and read back into the same [`Image`] (ADR 0007).
//!
//! The image lets the GPU be checked against the CPU rasterizer, the floor
//! path, on the same input (ADR 0013 D5.8). The viewport will present the
//! same drawing straight to its window instead, since reading the image back
//! costs about 20 ms a frame on the reference machine (ADR 0007).
//! It draws the very segments the rasterizer traces ([`trace`]), as
//! one-pixel lines, within wgpu's downlevel limits (what OpenGL ES 3.0
//! offers); 4× multisampling smooths them where the adapter offers it.
//! Dashes are cut on the CPU, as tiny-skia cuts them: each segment from its
//! start.

use std::sync::mpsc;
use std::time::Duration;

use crate::camera::Camera;
use crate::gpu_probe::{GpuBackend, GpuProbeResult, offscreen_adapter};
use crate::raster::{DASH_PX, Image, Sink, Style, trace};
use crate::scene::Scene;

/// The image the GPU draws: 8 bits per channel, no sRGB conversion, so
/// colors come back byte for byte.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// How long a frame may take on the GPU before the renderer gives up.
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
/// Bytes per vertex: x, y as `f32` and the color as four bytes.
const VERTEX_BYTES: u64 = 12;

const SHADER: &str = r"
struct View { size: vec2<f32>, unused: vec2<f32> };
@group(0) @binding(0) var<uniform> view: View;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Screen pixels (origin top-left, y down) to clip space.
@vertex
fn vs_main(@location(0) at: vec2<f32>, @location(1) color: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(
        at.x / view.size.x * 2.0 - 1.0,
        1.0 - at.y / view.size.y * 2.0,
        0.0,
        1.0,
    );
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return in.color;
}
";

/// A scene drawn offscreen on one GPU adapter.
pub struct GpuRenderer {
    adapter: GpuProbeResult,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    samples: u32,
    max_buffer: u64,
    vertices: Option<wgpu::Buffer>,
    target: Option<Target>,
    traced: Vec<Vertex>,
}

/// The textures and the readback buffer for one size.
struct Target {
    width: u32,
    height: u32,
    /// Bytes per row in the readback buffer, padded as wgpu requires.
    row_bytes: u32,
    /// The image read back: the multisampled texture's resolve, or the
    /// texture drawn when there is no multisampling.
    resolved: wgpu::Texture,
    multisampled: Option<wgpu::Texture>,
    readback: wgpu::Buffer,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Vertex {
    at: [f32; 2],
    rgba: [u8; 4],
}

impl GpuRenderer {
    /// A renderer on `backend`'s offscreen adapter, the one its probe
    /// checks. Run the probe in a child process first (ADR 0007): a driver
    /// can crash where an error cannot be caught.
    pub fn new(backend: GpuBackend) -> Result<Self, String> {
        pollster::block_on(Self::create(backend))
    }

    async fn create(backend: GpuBackend) -> Result<Self, String> {
        let adapter = offscreen_adapter(backend).await?;
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
        let samples = if adapter
            .get_texture_format_features(FORMAT)
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
                visibility: wgpu::ShaderStages::VERTEX,
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ArqStudioCAD renderer lines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ArqStudioCAD renderer view size"),
            size: 16,
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

        Ok(Self {
            adapter: GpuProbeResult {
                backend: backend.name().to_owned(),
                adapter: info.name,
                device_type: format!("{:?}", info.device_type),
            },
            device,
            queue,
            pipeline,
            uniform,
            bind_group,
            samples,
            max_buffer,
            vertices: None,
            target: None,
            traced: Vec::new(),
        })
    }

    /// The backend and adapter it draws on.
    pub fn adapter(&self) -> &GpuProbeResult {
        &self.adapter
    }

    /// Samples per pixel: 4 where the adapter offers multisampling, else 1.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Paints `scene` as `camera` sees it, like [`crate::render`]. An error
    /// means the GPU could not draw this frame; the caller draws it on the
    /// CPU instead.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: Style,
    ) -> Result<Image, String> {
        let (width, height) = camera.size();
        let mut traced = std::mem::take(&mut self.traced);
        traced.clear();
        let mut sink = VertexSink {
            vertices: &mut traced,
            rgba: [0, 0, 0, 255],
            dashed: false,
        };
        trace(scene, camera, style, &mut sink);
        let image = self.draw(&traced, width, height, style);
        self.traced = traced;
        image
    }

    fn draw(
        &mut self,
        vertices: &[Vertex],
        width: u32,
        height: u32,
        style: Style,
    ) -> Result<Image, String> {
        let bytes = vertices.len() as u64 * VERTEX_BYTES;
        if bytes > self.max_buffer {
            return Err(format!(
                "{} segments exceed the GPU's {}-byte buffers",
                vertices.len() / 2,
                self.max_buffer
            ));
        }
        self.ensure_target(width, height);
        self.ensure_vertices(bytes.max(VERTEX_BYTES * 2));
        let (Some(target), Some(buffer)) = (&self.target, &self.vertices) else {
            unreachable!("both made just above");
        };

        if !vertices.is_empty() {
            let mut data = Vec::with_capacity(bytes as usize);
            for v in vertices {
                data.extend_from_slice(&v.at[0].to_ne_bytes());
                data.extend_from_slice(&v.at[1].to_ne_bytes());
                data.extend_from_slice(&v.rgba);
            }
            self.queue.write_buffer(buffer, 0, &data);
        }
        let size = [width as f32, height as f32, 0.0, 0.0];
        let size: Vec<u8> = size.iter().flat_map(|v| v.to_ne_bytes()).collect();
        self.queue.write_buffer(&self.uniform, 0, &size);

        let resolved = target.resolved.create_view(&Default::default());
        let multisampled = target
            .multisampled
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        let (view, resolve_target) = match &multisampled {
            Some(m) => (m, Some(&resolved)),
            None => (&resolved, None),
        };
        let [r, g, b] = style.background.map(|c| f64::from(c) / 255.0);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ArqStudioCAD renderer frame"),
            });
        {
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
            if !vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..bytes));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }
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
        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    /// Textures and readback for `width` × `height`, kept between frames.
    fn ensure_target(&mut self, width: u32, height: u32) {
        if self
            .target
            .as_ref()
            .is_some_and(|t| t.width == width && t.height == height)
        {
            return;
        }
        let texture = |samples: u32, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ArqStudioCAD renderer image"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let resolved = texture(
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let multisampled = (self.samples > 1)
            .then(|| texture(self.samples, wgpu::TextureUsages::RENDER_ATTACHMENT));
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let row_bytes = (width * 4).div_ceil(align) * align;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ArqStudioCAD renderer readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.target = Some(Target {
            width,
            height,
            row_bytes,
            resolved,
            multisampled,
            readback,
        });
    }

    /// A vertex buffer of at least `bytes`, grown by half again when short.
    fn ensure_vertices(&mut self, bytes: u64) {
        if self.vertices.as_ref().is_some_and(|b| b.size() >= bytes) {
            return;
        }
        let size = (bytes + bytes / 2).min(self.max_buffer).max(bytes);
        self.vertices = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ArqStudioCAD renderer segments"),
            size: size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
}

/// Traced segments as line vertices, dashes cut here.
struct VertexSink<'a> {
    vertices: &'a mut Vec<Vertex>,
    rgba: [u8; 4],
    dashed: bool,
}

impl Sink for VertexSink<'_> {
    fn style(&mut self, rgb: [u8; 3], dashed: bool) {
        self.rgba = [rgb[0], rgb[1], rgb[2], 255];
        self.dashed = dashed;
    }

    fn segment(&mut self, a: (f32, f32), b: (f32, f32)) {
        if !self.dashed {
            self.push(a, b);
            return;
        }
        // On and off along the segment from its start, as tiny-skia dashes
        // a path of separate segments.
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx.hypot(dy);
        if length == 0.0 {
            return;
        }
        let at = |d: f32| (a.0 + dx * d / length, a.1 + dy * d / length);
        let [on, off] = DASH_PX;
        let mut d = 0.0;
        while d < length {
            self.push(at(d), at((d + on).min(length)));
            d += on + off;
        }
    }
}

impl VertexSink<'_> {
    fn push(&mut self, a: (f32, f32), b: (f32, f32)) {
        self.vertices.push(Vertex {
            at: [a.0, a.1],
            rgba: self.rgba,
        });
        self.vertices.push(Vertex {
            at: [b.0, b.1],
            rgba: self.rgba,
        });
    }
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
