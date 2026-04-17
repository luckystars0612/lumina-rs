//! Lumina-RS: Context-aware GPU particle engine for lofi video overlays
//!
//! This engine generates 10-second seamless loop particle effects that are
//! context-aware of the background image using WGPU compute shaders.

mod analysis;
mod config;
mod export;
mod shaders;

use anyhow::Result;
use config::{hex_to_rgba, IntentWrapper, OverlayConfig};
use export::{EncodingProgress, FFmpegConfig, FFmpegEncoder};
use image::{DynamicImage, ImageBuffer, Luma};
use shaders::{RenderParams, SimParams};
use std::path::Path;
use wgpu::{BlendState, ColorTargetState, ColorWrites, TextureFormat};

/// Lumina-RS engine state
pub struct LuminaEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    particle_buffer: wgpu::Buffer,
    luminance_texture: wgpu::Texture,
    background_texture: wgpu::Texture,
    output_texture: wgpu::Texture,
    sim_params_buffer: wgpu::Buffer,
    render_params_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    particle_count: usize,
}

impl LuminaEngine {
    /// Create a new engine instance
    pub async fn new(width: u32, height: u32, particle_count: usize) -> Result<Self> {
        // Initialize WGPU with Vulkan/DX12 for GTX 1070
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to request GPU adapter"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await?;

        // Create shaders
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute"),
            source: wgpu::ShaderSource::Wgsl(shaders::COMPUTE_SHADER.into()),
        });

        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render"),
            source: wgpu::ShaderSource::Wgsl(shaders::RENDER_SHADER.into()),
        });

        // Compute pipeline
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute"),
            layout: None,
            module: &compute_shader,
            entry_point: "main",
            compilation_options: Default::default(),
        });

        // Create bind group layout for render shader
        let render_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render_bind_group_layout"),
            entries: &[
                // binding 0: uniform (RenderParams)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: storage buffer (particles)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: texture (background)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: texture (luminance mask)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Render pipeline layout
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render_layout"),
            bind_group_layouts: &[&render_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // Particle buffer (storage buffer)
        let particle_size = std::mem::size_of::<Particle>() * particle_count;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles"),
            size: particle_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Luminance texture
        let luminance_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("luminance"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[TextureFormat::R8Unorm],
        });

        // Background texture
        let background_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("background"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[TextureFormat::Rgba8Unorm],
        });

        // Output texture for final render
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[TextureFormat::Rgba8Unorm],
        });

        // Uniform buffers
        let sim_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim_params"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let render_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_params"),
            size: std::mem::size_of::<RenderParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler for render shader
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create compute bind group (binding 0-3)
        let compute_bind_group_layout = compute_pipeline.get_bind_group_layout(0);
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute_bind_group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &luminance_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        // Create render bind group (binding 0-4)
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render_bind_group"),
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: render_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &background_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        &luminance_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            render_pipeline,
            particle_buffer,
            luminance_texture,
            background_texture,
            output_texture,
            sim_params_buffer,
            render_params_buffer,
            compute_bind_group,
            render_bind_group,
            width,
            height,
            particle_count,
        })
    }

    /// Update luminance mask from image
    pub fn update_luminance_mask(&self, mask: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<()> {
        let (width, height) = mask.dimensions();
        let data = mask.as_raw();

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.luminance_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Update background texture from image
    pub fn update_background(&self, img: &DynamicImage) -> Result<()> {
        let rgb = img.to_rgba8();
        let (width, height) = (img.width(), img.height());
        let data = rgb.as_raw();

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.background_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Render a single frame
    pub fn render_frame(&self, sim_params: &SimParams, render_params: &RenderParams) -> Result<Vec<u8>> {
        // Update uniform buffers
        self.queue.write_buffer(&self.sim_params_buffer, 0, bytemuck::bytes_of(sim_params));
        self.queue.write_buffer(&self.render_params_buffer, 0, bytemuck::bytes_of(render_params));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_encoder"),
        });

        // Compute pass
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                (self.particle_count as u32 + 63) / 64,
                1,
                1,
            );
        }

        // Render pass
        {
            let output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.render_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back output texture
        let bytes_per_row = self.width * 4;  // RGBA = 4 bytes per pixel
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: (bytes_per_row * self.height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let buffer_slice = staging_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range().to_vec();

        Ok(data)
    }
}

/// Particle data structure (must match shader)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Particle {
    position: [f32; 3],  // xy = screen pos, z = depth
    velocity: [f32; 3],  // current velocity
    seed: [f32; 2],      // random seed
    lifetime: f32,       // 0-1
    size: f32,           // particle size
    phase: f32,          // animation phase
}

/// Render a 10-second loop video
pub async fn render_loop(
    background_path: &Path,
    config: &OverlayConfig,
    output_path: &str,
) -> Result<()> {
    let width = 1920u32;
    let height = 1080u32;
    let fps = 60u32;
    let total_frames = 600u32; // 10 seconds @ 60fps
    let particle_count = 5_000usize; // Further reduced for cleaner visuals

    // Load background image
    let background = image::open(background_path)?;
    let background = background.resize_exact(width, height, image::imageops::FilterType::Lanczos3);

    // Create luminance mask
    let (mask, _) = analysis::load_and_analyze(background_path)?;
    
    // Resize luminance mask to match fixed GPU dimensions (1920x1080)
    let mask = image::imageops::resize(
        &mask,
        width,
        height,
        image::imageops::FilterType::Lanczos3
    );

    // Create engine
    let engine = LuminaEngine::new(width, height, particle_count).await?;

    // Upload textures (mask is now resized to 1920x1080)
    engine.update_luminance_mask(&mask)?;
    engine.update_background(&background)?;

    // Convert config to shader params
    let params = &config.params;
    
    // Rain type to shader index
    let rain_type_idx = match params.rain_type {
        Some(config::RainType::Drizzle) => 0u32,
        Some(config::RainType::Normal) => 1u32,
        Some(config::RainType::Heavy) => 2u32,
        Some(config::RainType::Storm) => 3u32,
        None => 1u32, // default to normal
    };
    
    let sim_params = SimParams {
        delta_time: 1.0 / fps as f32,
        time: 0.0,
        width: width as f32,
        height: height as f32,
        preset: config.preset.to_shader_index(),
        density: params.density_multiplier,
        velocity_min: params.velocity_scale[0],
        velocity_max: params.velocity_scale[1],
        turbulence: params.turbulence,
        flicker_speed: params.flicker_speed,
        size_min: params.size_range[0],
        size_max: params.size_range[1],
        sway_intensity: 0.5,
        buoyancy_force: 0.0,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
        base_color: hex_to_rgba(&params.base_color_hex),
        // Rain-specific parameters
        wind_direction: params.wind_direction,
        wind_strength: params.wind_strength,
        gust_enabled: if params.gust_enabled { 1.0 } else { 0.0 },
        gust_frequency: params.gust_frequency,
        gust_strength: params.gust_strength,
        gust_duration: params.gust_duration,
        rain_type: rain_type_idx,
        splash_enabled: if params.splash_enabled { 1.0 } else { 0.0 },
        splash_velocity: params.splash_velocity,
        _pad3: 0.0,
        _pad4: 0.0,
    };

    let render_params = RenderParams {
        time: 0.0,
        width: width as f32,
        height: height as f32,
        preset: config.preset.to_shader_index(),
        flicker_speed: params.flicker_speed,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
        base_color: hex_to_rgba(&params.base_color_hex),
        _pad3: 0.0,
        _pad4: 0.0,
        _pad5: 0.0,
        _pad6: 0.0,
    };

    // Create FFmpeg encoder
    let ffmpeg_config = FFmpegConfig {
        output_path: output_path.to_string(),
        width,
        height,
        fps,
        cq: 22,
    };
    let mut encoder = FFmpegEncoder::new(ffmpeg_config)?;
    let mut progress = EncodingProgress::new(total_frames);

    println!("Rendering {} frames ({} seconds @ {}fps)...", total_frames, total_frames / fps, fps);
    println!("Preset: {:?}", config.preset);
    println!("Base color: {}", params.base_color_hex);

    // Render loop
    for frame in 0..total_frames {
        let loop_time = (frame as f32 / fps as f32) % 10.0; // 10-second seamless loop

        // Update time params
        let mut sim_params = sim_params;
        sim_params.time = loop_time;

        let mut render_params = render_params;
        render_params.time = loop_time;

        // Render frame
        let rgba_data = engine.render_frame(&sim_params, &render_params)?;
        encoder.write_frame(&rgba_data)?;

        progress.update();
        if frame % 60 == 0 {
            println!("Progress: {:.1}% ({}/{})", progress.percentage(), progress.current_frame, progress.total_frames);
        }
    }

    // Finish encoding
    encoder.finish()?;
    println!("Done! Video saved to: {}", output_path);

    Ok(())
}

fn main() -> Result<()> {
    // Check for input args
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 3 {
        println!("Lumina-RS: Context-aware Particle Engine");
        println!();
        println!("Usage:");
        println!("  lumina-core <background_image> <intent_json> [output_video]");
        println!();
        println!("Examples:");
        println!("  lumina-core resources/images/image1.png resources/intent/intent1.json rain_output.mp4");
        println!("  lumina-core background.png config.json output.mp4");
        println!();
        println!("Available presets: normal, rain, snow, fireflies, sun_dust, embers, petals");
        return Ok(());
    }

    let background_path = Path::new(&args[1]);
    let intent_path = Path::new(&args[2]);
    let output_path = args.get(3).map(|s| s.as_str()).unwrap_or("output.mp4");

    // Load intent config (try nested format first, then flat format)
    let intent_json = std::fs::read_to_string(intent_path)?;
    let wrapper: IntentWrapper = match serde_json::from_str(&intent_json) {
        Ok(w) => w,
        Err(_) => {
            // Try flat format
            let config: config::OverlayConfig = serde_json::from_str(&intent_json)?;
            return pollster::block_on(render_loop(background_path, &config, output_path));
        }
    };
    
    println!("Loaded intent: {:?}", wrapper.overlay_config.preset);
    println!("Background: {:?}", background_path);
    println!("Output: {}", output_path);

    // Run render loop
    pollster::block_on(render_loop(background_path, &wrapper.overlay_config, output_path))?;

    Ok(())
}
