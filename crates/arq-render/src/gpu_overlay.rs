//! The overlays on the GPU (ADR 0034 D3): the triangles of
//! [`overlay::triangles`](crate::overlay) with their edges faded in the
//! shader, then the texts Qt painted as textured quads, both blended over
//! the drawing in the same pass.

use crate::overlay::{Overlay, OverlayVertex, triangles};

/// Bytes per overlay vertex: position, color, across and half.
const OVERLAY_BYTES: u64 = 20;
/// Bytes per label vertex: position and texture coordinate.
const LABEL_BYTES: u64 = 16;

const SHADER: &str = r"
struct View {
    center: vec2<f32>,
    size: vec2<f32>,
    scale: f32,
    dash_on: f32,
    dash_period: f32,
    unused: f32,
};
@group(0) @binding(0) var<uniform> view: View;

fn clip(px: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(px.x / view.size.x * 2.0 - 1.0, 1.0 - px.y / view.size.y * 2.0, 0.0, 1.0);
}

struct Stroke {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) across: f32,
    @location(2) half: f32,
};

@vertex
fn vs_overlay(
    @location(0) at: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) across: f32,
    @location(3) half: f32,
) -> Stroke {
    var out: Stroke;
    out.position = clip(at);
    out.color = color;
    out.across = across;
    out.half = half;
    return out;
}

// Premultiplied out: a stroke's edge fades over its last half pixel each
// side; fills and crisp strokes (half < 0) are never faded.
@fragment
fn fs_overlay(in: Stroke) -> @location(0) vec4<f32> {
    var alpha = in.color.a;
    if (in.half >= 0.0) {
        alpha = alpha * clamp(in.half + 0.5 - abs(in.across), 0.0, 1.0);
    }
    return vec4<f32>(in.color.rgb * alpha, alpha);
}

@group(1) @binding(0) var label_image: texture_2d<f32>;
@group(1) @binding(1) var label_sampler: sampler;

struct Label {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_label(@location(0) at: vec2<f32>, @location(1) uv: vec2<f32>) -> Label {
    var out: Label;
    out.position = clip(at);
    out.uv = uv;
    return out;
}

// Qt paints texts premultiplied: drawn as they come.
@fragment
fn fs_label(in: Label) -> @location(0) vec4<f32> {
    return textureSample(label_image, label_sampler, in.uv);
}
";

/// What it takes to draw the overlays, and this frame's.
pub(crate) struct OverlayPainter {
    shader: wgpu::ShaderModule,
    stroke_layout: wgpu::PipelineLayout,
    label_layout: wgpu::PipelineLayout,
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    samples: u32,
    pipelines: Vec<(
        wgpu::TextureFormat,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    )>,
    vertices: Option<wgpu::Buffer>,
    count: u32,
    labels: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
}

impl OverlayPainter {
    pub(crate) fn new(
        device: &wgpu::Device,
        view_layout: &wgpu::BindGroupLayout,
        samples: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ArqStudioCAD overlay shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ArqStudioCAD overlay text"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let stroke_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ArqStudioCAD overlay strokes"),
            bind_group_layouts: &[Some(view_layout)],
            immediate_size: 0,
        });
        let label_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ArqStudioCAD overlay texts"),
            bind_group_layouts: &[Some(view_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        // Nearest: a text is drawn at whole pixels, as Qt painted it.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Self {
            shader,
            stroke_layout,
            label_layout,
            image_layout,
            sampler,
            samples,
            pipelines: Vec::new(),
            vertices: None,
            count: 0,
            labels: Vec::new(),
        }
    }

    /// Uploads this frame's overlay and makes the pipelines for `format`.
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        overlay: &Overlay,
        format: wgpu::TextureFormat,
    ) {
        self.ensure_pipelines(device, format);
        let vertices = triangles(overlay);
        self.count = vertices.len() as u32;
        if !vertices.is_empty() {
            let data = stroke_bytes(&vertices);
            let size = data.len() as u64;
            if !self.vertices.as_ref().is_some_and(|b| b.size() >= size) {
                self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ArqStudioCAD overlay"),
                    size: (size + size / 2).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            if let Some(buffer) = &self.vertices {
                queue.write_buffer(buffer, 0, &data);
            }
        }
        // Texts are few and small: made again each frame.
        self.labels.clear();
        for label in &overlay.labels {
            if label.width == 0 || label.height == 0 {
                continue;
            }
            let size = wgpu::Extent3d {
                width: label.width,
                height: label.height,
                depth_or_array_layers: 1,
            };
            let image = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ArqStudioCAD overlay text"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                image.as_image_copy(),
                &label.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(label.width * 4),
                    rows_per_image: Some(label.height),
                },
                size,
            );
            let view = image.create_view(&Default::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ArqStudioCAD overlay text"),
                layout: &self.image_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            // Labels come in physical pixels, at whole ones.
            let [x, y] = label.at.map(|v| v as f32);
            let (w, h) = (label.width as f32, label.height as f32);
            let corners = [
                [x, y, 0.0, 0.0],
                [x + w, y, 1.0, 0.0],
                [x + w, y + h, 1.0, 1.0],
                [x, y + h, 0.0, 1.0],
            ];
            let mut data = Vec::with_capacity(6 * LABEL_BYTES as usize);
            for i in [0, 1, 2, 0, 2, 3] {
                for f in corners[i] {
                    data.extend_from_slice(&f.to_ne_bytes());
                }
            }
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ArqStudioCAD overlay text quad"),
                size: data.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &data);
            self.labels.push((buffer, bind_group));
        }
    }

    /// Draws what [`Self::prepare`] uploaded, over what the pass drew.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, format: wgpu::TextureFormat) {
        let Some((_, strokes, texts)) = self.pipelines.iter().find(|(f, _, _)| *f == format) else {
            return;
        };
        if let (Some(buffer), true) = (&self.vertices, self.count > 0) {
            pass.set_pipeline(strokes);
            pass.set_vertex_buffer(0, buffer.slice(..u64::from(self.count) * OVERLAY_BYTES));
            pass.draw(0..self.count, 0..1);
        }
        if !self.labels.is_empty() {
            pass.set_pipeline(texts);
            for (buffer, bind_group) in &self.labels {
                pass.set_bind_group(1, bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        }
    }

    fn ensure_pipelines(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.iter().any(|(f, _, _)| *f == format) {
            return;
        }
        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let attribute = |format, offset, shader_location| wgpu::VertexAttribute {
            format,
            offset,
            shader_location,
        };
        let make = |layout: &wgpu::PipelineLayout,
                    entry: (&str, &str),
                    stride: u64,
                    attributes: &[wgpu::VertexAttribute]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ArqStudioCAD overlay"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(entry.0),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: self.samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(entry.1),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let strokes = make(
            &self.stroke_layout,
            ("vs_overlay", "fs_overlay"),
            OVERLAY_BYTES,
            &[
                attribute(wgpu::VertexFormat::Float32x2, 0, 0),
                attribute(wgpu::VertexFormat::Unorm8x4, 8, 1),
                attribute(wgpu::VertexFormat::Float32, 12, 2),
                attribute(wgpu::VertexFormat::Float32, 16, 3),
            ],
        );
        let texts = make(
            &self.label_layout,
            ("vs_label", "fs_label"),
            LABEL_BYTES,
            &[
                attribute(wgpu::VertexFormat::Float32x2, 0, 0),
                attribute(wgpu::VertexFormat::Float32x2, 8, 1),
            ],
        );
        self.pipelines.push((format, strokes, texts));
    }
}

fn stroke_bytes(vertices: &[OverlayVertex]) -> Vec<u8> {
    let mut data = Vec::with_capacity(vertices.len() * OVERLAY_BYTES as usize);
    for v in vertices {
        data.extend_from_slice(&v.at[0].to_ne_bytes());
        data.extend_from_slice(&v.at[1].to_ne_bytes());
        data.extend_from_slice(&v.rgba);
        data.extend_from_slice(&v.across.to_ne_bytes());
        data.extend_from_slice(&v.half.to_ne_bytes());
    }
    data
}
