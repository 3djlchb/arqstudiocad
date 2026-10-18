//! Isolated, offscreen checks for the graphics backends (ADR 0007).
//!
//! The caller runs this function in a short-lived child process. A driver
//! failure therefore cannot take down the main application.

use std::sync::mpsc;
use std::time::Duration;

/// A backend accepted by the runtime probe command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuBackend {
    /// Direct3D 12, including WARP when requesting a fallback adapter.
    Dx12,
    /// Vulkan.
    Vulkan,
    /// OpenGL or OpenGL ES through wgpu's GLES backend.
    OpenGl,
    /// Metal.
    Metal,
    /// The platform's software adapter (WARP on Windows, llvmpipe on Linux).
    Software,
}

impl GpuBackend {
    /// Parses the stable command-line backend name.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "dx12" => Ok(Self::Dx12),
            "vulkan" => Ok(Self::Vulkan),
            "opengl" => Ok(Self::OpenGl),
            "metal" => Ok(Self::Metal),
            "software" => Ok(Self::Software),
            _ => Err(format!("unsupported GPU backend: {name}")),
        }
    }

    /// Stable command-line name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
            Self::OpenGl => "opengl",
            Self::Metal => "metal",
            Self::Software => "software",
        }
    }

    fn wgpu_backend(self) -> wgpu::Backends {
        match self {
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::OpenGl => wgpu::Backends::GL,
            Self::Metal => wgpu::Backends::METAL,
            Self::Software => {
                #[cfg(target_os = "windows")]
                {
                    wgpu::Backends::DX12
                }
                #[cfg(target_os = "linux")]
                {
                    wgpu::Backends::GL
                }
                #[cfg(target_os = "macos")]
                {
                    wgpu::Backends::METAL
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                {
                    wgpu::Backends::all()
                }
            }
        }
    }

    fn force_fallback(self) -> bool {
        matches!(self, Self::Software)
    }
}

/// Result of creating a device, compiling a pipeline, drawing and reading back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuProbeResult {
    /// Backend that was checked.
    pub backend: String,
    /// Name reported by the graphics adapter.
    pub adapter: String,
    /// Adapter class reported by wgpu.
    pub device_type: String,
}

/// Verifies an adapter with a small offscreen draw and a readback.
///
/// Call this only in a child process with a parent-enforced timeout. Graphics
/// driver initialization can crash or block independently of Rust errors.
pub fn probe_gpu_backend(backend: GpuBackend) -> Result<GpuProbeResult, String> {
    pollster::block_on(probe(backend))
}

async fn probe(backend: GpuBackend) -> Result<GpuProbeResult, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = backend.wgpu_backend();
    let instance = wgpu::Instance::new(descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: backend.force_fallback(),
            compatible_surface: None,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|error| error.to_string())?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|error| error.to_string())?;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ArqStudioCAD GPU probe target"),
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ArqStudioCAD GPU probe shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
                @vertex
                fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
                    var positions = array<vec2<f32>, 3>(
                        vec2<f32>(-1.0, -1.0),
                        vec2<f32>( 3.0, -1.0),
                        vec2<f32>(-1.0,  3.0),
                    );
                    return vec4<f32>(positions[index], 0.0, 1.0);
                }

                @group(0) @binding(0) var<uniform> probe_color: vec4<f32>;

                @fragment
                fn fs_main() -> @location(0) vec4<f32> {
                    return probe_color;
                }
            "#
            .into(),
        ),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ArqStudioCAD GPU probe bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ArqStudioCAD GPU probe pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ArqStudioCAD GPU probe pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let row_bytes = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ArqStudioCAD GPU probe readback"),
        size: row_bytes * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    // The shader consumes this value, so readback checks the upload too.
    let upload = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ArqStudioCAD GPU probe upload"),
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let upload_data = [
        1.0_f32.to_ne_bytes(),
        0.0_f32.to_ne_bytes(),
        0.0_f32.to_ne_bytes(),
        1.0_f32.to_ne_bytes(),
    ]
    .concat();
    queue.write_buffer(&upload, 0, &upload_data);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ArqStudioCAD GPU probe bind group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: upload.as_entire_binding(),
        }],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ArqStudioCAD GPU probe commands"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ArqStudioCAD GPU probe pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes as u32),
                rows_per_image: Some(4),
            },
        },
        wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);

    let (sender, receiver) = mpsc::channel();
    output
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(5)),
        })
        .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let pixels = output
        .slice(..)
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    let center = (row_bytes * 2 + 2 * 4) as usize;
    let pixel = &pixels[center..center + 4];
    if pixel != [255, 0, 0, 255] {
        return Err(format!("unexpected offscreen pixel: {pixel:?}"));
    }
    drop(pixels);
    output.unmap();

    Ok(GpuProbeResult {
        backend: backend.name().to_owned(),
        adapter: info.name,
        device_type: format!("{:?}", info.device_type),
    })
}

#[cfg(test)]
mod tests {
    use super::GpuBackend;

    #[test]
    fn parses_stable_backend_names() {
        assert_eq!(GpuBackend::parse("dx12"), Ok(GpuBackend::Dx12));
        assert_eq!(GpuBackend::parse("vulkan"), Ok(GpuBackend::Vulkan));
        assert_eq!(GpuBackend::parse("opengl"), Ok(GpuBackend::OpenGl));
        assert_eq!(GpuBackend::parse("metal"), Ok(GpuBackend::Metal));
        assert_eq!(GpuBackend::parse("software"), Ok(GpuBackend::Software));
        assert!(GpuBackend::parse("unknown").is_err());
    }
}
