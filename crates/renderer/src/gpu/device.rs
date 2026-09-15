//! wgpu device wrapper: uploads packed triangle bins, runs the exact-fill compute shader into an
//! rgba8 storage texture, presents that texture to a surface and records genuine GPU completion.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use wgpu::util::DeviceExt;

use super::pack::PackedFrame;
use crate::error::RenderError;
use crate::raster::RasterState;
use crate::texture::TextureSet;

/// Texture atlas layout for the shader: all 128x128 texel arrays concatenated plus an id table.
pub struct GpuTextures {
    pub texels: Vec<u32>,
    pub table: Vec<u32>,
    pub ids: Vec<i32>,
}

impl GpuTextures {
    pub fn from_set(set: &TextureSet) -> Self {
        let mut ids: Vec<i32> = set.ids().collect();
        ids.sort_unstable();
        let max_id = ids.iter().copied().max().unwrap_or(0).max(0) as usize;
        let mut table = vec![0u32; max_id + 1];
        let mut texels = Vec::with_capacity(ids.len() * 128 * 128);
        for &id in &ids {
            let texture = set.get(id).expect("listed id");
            let base = texels.len() as u32;
            texels.extend(texture.pixels.iter().map(|&p| p as u32));
            table[id as usize] = (base + 1) | if texture.opaque { 0x4000_0000 } else { 0 };
        }
        if texels.is_empty() {
            texels.push(0);
        }
        Self { texels, table, ids }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    bins_x: u32,
    bins_y: u32,
    center_x: i32,
    center_y: i32,
    zoom: i32,
    clear_color: u32,
}

/// Shared completion flag set from the queue's submitted-work-done callback.
#[derive(Clone)]
pub struct GpuFrame {
    pub sequence: u64,
    done: Arc<AtomicBool>,
    gpu_duration_ns: Arc<Mutex<Option<u64>>>,
}

impl GpuFrame {
    pub fn is_complete(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }
    /// GPU timestamp span in nanoseconds when the device supports timestamp queries and the
    /// resolve buffer has been read back.
    pub fn gpu_duration_ns(&self) -> Option<u64> {
        *self.gpu_duration_ns.lock().expect("lock")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameRecord {
    pub triangles: usize,
    pub bin_entries: usize,
    pub draw_calls: usize,
    pub texture_fallbacks: usize,
}

pub struct GpuRasterizer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    blit_layout: wgpu::BindGroupLayout,
    blit_pipeline: Option<(wgpu::RenderPipeline, wgpu::TextureFormat)>,
    sampler: wgpu::Sampler,
    palette: wgpu::Buffer,
    texels: wgpu::Buffer,
    texture_table: wgpu::Buffer,
    params: wgpu::Buffer,
    tris: wgpu::Buffer,
    bin_offsets: wgpu::Buffer,
    bin_tris: wgpu::Buffer,
    output: wgpu::Texture,
    output_view: wgpu::TextureView,
    width: u32,
    height: u32,
    sequence: u64,
    timestamps: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer, f32)>,
    pub last_record: FrameRecord,
}

const SHADER: &str = include_str!("raster.wgsl");
const BLIT_SHADER: &str = r#"
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (y + 1.0) * 0.5);
    return out;
}
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@fragment fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(src, samp, in.uv);
}
"#;

impl GpuRasterizer {
    pub fn new(
        device: wgpu::Device, queue: wgpu::Queue, palette: &[i32], textures: &GpuTextures, width: u32, height: u32,
    ) -> Result<Self, RenderError> {
        if palette.len() != 65536 {
            return Err(RenderError::InvalidAsset("palette must have 65536 entries".into()));
        }
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clubscape-exact-raster"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let storage = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clubscape-raster-layout"),
            entries: &[
                storage(0),
                storage(1),
                storage(2),
                storage(3),
                storage(4),
                storage(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("clubscape-raster-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("clubscape-raster-pipeline"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clubscape-blit-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("clubscape-blit-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let palette_bytes: Vec<u8> = palette.iter().flat_map(|v| (*v as u32).to_le_bytes()).collect();
        let palette_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("clubscape-palette"),
            contents: &palette_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let texel_bytes: Vec<u8> = textures.texels.iter().flat_map(|v| v.to_le_bytes()).collect();
        let texels = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("clubscape-texels"),
            contents: &texel_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let table: Vec<u32> = if textures.table.is_empty() { vec![0] } else { textures.table.clone() };
        let table_bytes: Vec<u8> = table.iter().flat_map(|v| v.to_le_bytes()).collect();
        let texture_table = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("clubscape-texture-table"),
            contents: &table_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clubscape-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tris = Self::storage_buffer(&device, "clubscape-tris", 1 << 20);
        let bin_offsets = Self::storage_buffer(&device, "clubscape-bin-offsets", 1 << 16);
        let bin_tris = Self::storage_buffer(&device, "clubscape-bin-tris", 1 << 18);
        let (output, output_view) = Self::create_output(&device, width, height);
        let timestamps = if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("clubscape-timestamps"), ty: wgpu::QueryType::Timestamp, count: 2 });
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("clubscape-timestamp-resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("clubscape-timestamp-read"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            Some((set, resolve, read, queue.get_timestamp_period()))
        } else {
            None
        };
        Ok(Self {
            device,
            queue,
            layout,
            pipeline,
            blit_layout,
            blit_pipeline: None,
            sampler,
            palette: palette_buffer,
            texels,
            texture_table,
            params,
            tris,
            bin_offsets,
            bin_tris,
            output,
            output_view,
            width,
            height,
            sequence: 0,
            timestamps,
            last_record: FrameRecord::default(),
        })
    }

    pub fn timestamps_supported(&self) -> bool {
        self.timestamps.is_some()
    }

    fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_output(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("clubscape-output"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = output.create_view(&wgpu::TextureViewDescriptor::default());
        (output, view)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        let (output, view) = Self::create_output(&self.device, width, height);
        self.output = output;
        self.output_view = view;
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn ensure_capacity(&mut self, packed: &PackedFrame) {
        let tri_bytes = (packed.tris.len().max(TRI_MIN) * 4) as u64;
        if self.tris.size() < tri_bytes {
            self.tris = Self::storage_buffer(&self.device, "clubscape-tris", tri_bytes.next_power_of_two());
        }
        let off_bytes = (packed.bin_offsets.len() * 4) as u64;
        if self.bin_offsets.size() < off_bytes {
            self.bin_offsets = Self::storage_buffer(&self.device, "clubscape-bin-offsets", off_bytes.next_power_of_two());
        }
        let bt_bytes = (packed.bin_tris.len().max(1) * 4) as u64;
        if self.bin_tris.size() < bt_bytes {
            self.bin_tris = Self::storage_buffer(&self.device, "clubscape-bin-tris", bt_bytes.next_power_of_two());
        }
    }

    /// Uploads the packed frame and dispatches the exact-fill compute pass. `state` supplies the
    /// projection center and zoom the triangles were produced with.
    pub fn render(&mut self, state: &RasterState, packed: &PackedFrame, clear_color: u32) -> Result<GpuFrame, RenderError> {
        if state.width as u32 != self.width || state.height as u32 != self.height {
            return Err(RenderError::Gpu(format!("frame size {}x{} does not match target {}x{}", state.width, state.height, self.width, self.height)));
        }
        self.ensure_capacity(packed);
        let params = Params {
            width: self.width,
            height: self.height,
            bins_x: packed.bins_x,
            bins_y: packed.bins_y,
            center_x: state.center_x,
            center_y: state.center_y,
            zoom: state.zoom,
            clear_color,
        };
        self.queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        if !packed.tris.is_empty() {
            self.queue.write_buffer(&self.tris, 0, bytemuck::cast_slice(&packed.tris));
        }
        self.queue.write_buffer(&self.bin_offsets, 0, bytemuck::cast_slice(&packed.bin_offsets));
        if !packed.bin_tris.is_empty() {
            self.queue.write_buffer(&self.bin_tris, 0, bytemuck::cast_slice(&packed.bin_tris));
        }
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clubscape-raster-bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.tris.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bin_offsets.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.bin_tris.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.palette.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: self.texels.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: self.texture_table.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: self.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.output_view) },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clubscape-frame") });
        {
            let timestamp_writes = self.timestamps.as_ref().map(|(set, _, _, _)| wgpu::ComputePassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("clubscape-exact-fill"), timestamp_writes });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(packed.bins_x, packed.bins_y, 1);
        }
        if let Some((set, resolve, read, _)) = &self.timestamps {
            encoder.resolve_query_set(set, 0..2, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, read, 0, 16);
        }
        self.queue.submit([encoder.finish()]);
        self.sequence += 1;
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        self.queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
        let gpu_duration_ns = Arc::new(Mutex::new(None));
        if let Some((_, _, read, period)) = &self.timestamps {
            let slot = gpu_duration_ns.clone();
            let period = *period;
            let read_clone = read.clone();
            read.map_async(wgpu::MapMode::Read, .., move |result| {
                if result.is_ok() {
                    if let Ok(view) = read_clone.get_mapped_range(..) {
                        let stamps: &[u64] = bytemuck::cast_slice(&view);
                        if stamps.len() >= 2 && stamps[1] >= stamps[0] {
                            let ns = ((stamps[1] - stamps[0]) as f64 * f64::from(period)) as u64;
                            *slot.lock().expect("lock") = Some(ns);
                        }
                    }
                    read_clone.unmap();
                }
            });
        }
        self.last_record = FrameRecord {
            triangles: packed.triangle_count,
            bin_entries: packed.bin_tris.len(),
            draw_calls: 1,
            texture_fallbacks: packed.texture_fallbacks,
        };
        Ok(GpuFrame { sequence: self.sequence, done, gpu_duration_ns })
    }

    /// Draws the completed output texture onto `target` (a surface texture view).
    pub fn blit(&mut self, target: &wgpu::TextureView, format: wgpu::TextureFormat) {
        if self.blit_pipeline.as_ref().map(|(_, f)| *f) != Some(format) {
            let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("clubscape-blit"),
                source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
            });
            let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("clubscape-blit-layout"),
                bind_group_layouts: &[Some(&self.blit_layout)],
                immediate_size: 0,
            });
            let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("clubscape-blit-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.blit_pipeline = Some((pipeline, format));
        }
        let (pipeline, _) = self.blit_pipeline.as_ref().expect("blit pipeline");
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clubscape-blit-bind"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.output_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clubscape-blit-encoder") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clubscape-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Copies the output texture into a mappable buffer and returns it as `0xRRGGBB` pixels.
    /// Blocks on native backends; not available on WebGPU (use the surface instead).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_back(&self) -> Result<Vec<i32>, RenderError> {
        let bytes_per_row = (self.width * 4).div_ceil(256) * 256;
        let size = u64::from(bytes_per_row) * u64::from(self.height);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clubscape-readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clubscape-readback-encoder") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.output, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None } },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| RenderError::Gpu(format!("poll: {e:?}")))?;
        rx.recv().map_err(|e| RenderError::Gpu(e.to_string()))?.map_err(|e| RenderError::Gpu(format!("map: {e:?}")))?;
        let view = buffer.get_mapped_range(..).map_err(|e| RenderError::Gpu(format!("mapped range: {e:?}")))?;
        let mut out = vec![0i32; (self.width * self.height) as usize];
        for y in 0..self.height as usize {
            let row = &view[y * bytes_per_row as usize..];
            for x in 0..self.width as usize {
                let p = &row[x * 4..x * 4 + 4];
                out[y * self.width as usize + x] = ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32;
            }
        }
        drop(view);
        buffer.unmap();
        Ok(out)
    }
}

const TRI_MIN: usize = 20;

/// Native helper: request a hardware adapter/device for tests and tools.
#[cfg(not(target_arch = "wasm32"))]
pub async fn request_native_device() -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::PRIMARY;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|e| RenderError::Gpu(format!("no adapter: {e}")))?;
    let info = adapter.get_info();
    if matches!(info.device_type, wgpu::DeviceType::Cpu) {
        return Err(RenderError::Gpu(format!("software adapter refused: {}", info.name)));
    }
    let mut features = wgpu::Features::empty();
    if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        features |= wgpu::Features::TIMESTAMP_QUERY;
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("clubscape-renderer"),
            required_features: features,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|e| RenderError::Gpu(format!("device: {e}")))?;
    Ok((adapter, device, queue))
}
