use anyhow::Result;
use glam::Vec3;
use image::{ImageBuffer, Rgba};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{event::WindowEvent, window::Window};

use crate::{
    camera::{Camera, CameraUniform, Projection},
    debug::Debug,
    game_object::{
        DrawLight, DrawModel, GameObjectStore, InstanceRaw, LightUniform, ModelVertex, Transform3D,
        Vertex,
    },
    hdr::HdrPipeline,
    texture::Texture,
    MAX_INSTANCES,
};

pub enum RenderTarget {
    Window(Arc<Window>),
    Headless { width: u32, height: u32 },
}

pub enum RenderOutput {
    Surface {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Buffer {
        width: u32,
        height: u32,
        padded_bytes_per_row: u32,
        texture: wgpu::Texture,
        buffer: wgpu::Buffer,
    },
}

#[allow(dead_code)]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    output: RenderOutput,
    render_pipeline: wgpu::RenderPipeline,
    light_render_pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    depth_texture: Texture,
    hdr: HdrPipeline,
    pub camera: Camera,
    projection: Projection,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    light_bind_group_layout: wgpu::BindGroupLayout,
    light_bind_group: wgpu::BindGroup,
    pub debug: Debug,
}

impl Renderer {
    pub async fn new(target: RenderTarget) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let (surface, window, (width, height)) = match target {
            RenderTarget::Window(window) => {
                let surface = instance.create_surface(window.clone()).unwrap();
                let size = window.inner_size();
                (Some(surface), Some(window), (size.width, size.height))
            }
            RenderTarget::Headless { width, height } => (None, None, (width, height)),
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let (output, format) = if let Some(surface) = surface
            && let Some(window) = window
        {
            let surface_caps = surface.get_capabilities(&adapter);

            let format = surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width,
                height,
                present_mode: wgpu::PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
            };

            surface.configure(&device, &config);

            (
                RenderOutput::Surface {
                    window,
                    surface,
                    config,
                },
                format,
            )
        } else {
            let texture_desc = wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
                label: None,
                view_formats: &[],
            };
            let texture = device.create_texture(&texture_desc);

            let u32_size = std::mem::size_of::<u32>() as u32;

            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let unpadded_bytes_per_row = u32_size * width;
            let padding = (align - unpadded_bytes_per_row % align) % align;
            let padded_bytes_per_row = unpadded_bytes_per_row + padding;

            let output_buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;
            let output_buffer_desc = wgpu::BufferDescriptor {
                size: output_buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                label: None,
                mapped_at_creation: false,
            };
            let buffer = device.create_buffer(&output_buffer_desc);

            (
                RenderOutput::Buffer {
                    width,
                    height,
                    padded_bytes_per_row,
                    texture,
                    buffer,
                },
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        };

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let depth_texture = Texture::create_depth_texture(&device, width, height, "depth_texture");

        let hdr = HdrPipeline::new(&device, width, height, format);

        let camera = Camera::new(
            [0.0, 5.0, 10.0],
            -90.0_f32.to_radians(),
            -20.0_f32.to_radians(),
        );
        let projection = Projection::new(width, height, 45.0_f32.to_radians(), 0.1, 1000000.0);

        let mut camera_uniform = CameraUniform::default();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let instances = (0..MAX_INSTANCES).map(move |_| Transform3D {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        });

        let instance_data: Vec<InstanceRaw> = instances
            .map(|i| i.to_raw_instance())
            .collect::<Vec<InstanceRaw>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transform3D Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let light_uniform = LightUniform::default();
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("light_bind_group_layout"),
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: Some("light_bind_group"),
        });

        let render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shader.wgsl").into()),
            };

            Self::create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(Texture::DEPTH_FORMAT),
                &[ModelVertex::desc(), InstanceRaw::desc()],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
                push_constant_ranges: &[],
            });

            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/light.wgsl").into()),
            };

            Self::create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(Texture::DEPTH_FORMAT),
                &[ModelVertex::desc(), InstanceRaw::desc()],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        let debug = Debug::new(&device, &camera_bind_group_layout, format);

        Self {
            device,
            queue,
            output,
            render_pipeline,
            light_render_pipeline,
            texture_bind_group_layout,
            depth_texture,
            hdr,
            camera,
            projection,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            instance_buffer,
            light_buffer,
            light_bind_group_layout,
            light_bind_group,
            debug,
        }
    }

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        topology: wgpu::PrimitiveTopology,
        shader: wgpu::ShaderModuleDescriptor,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(shader);

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.projection.resize(new_size.width, new_size.height);
            self.hdr
                .resize(&self.device, new_size.width, new_size.height);

            match &mut self.output {
                RenderOutput::Surface {
                    surface, config, ..
                } => {
                    config.width = new_size.width;
                    config.height = new_size.height;
                    surface.configure(&self.device, config);
                }
                RenderOutput::Buffer { width, height, .. } => {
                    *width = new_size.width;
                    *height = new_size.height;
                }
            }

            self.depth_texture = Texture::create_depth_texture(
                &self.device,
                new_size.width,
                new_size.height,
                "depth_texture",
            );
        }
    }

    pub fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    pub fn render(&mut self, game_objects: &GameObjectStore) -> Result<(), wgpu::SurfaceError> {
        let pre_frame_data = game_objects.pre_frame();

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[pre_frame_data.light_uniform]),
        );

        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&pre_frame_data.instances),
        );

        let (view, surface_texture) = match &mut self.output {
            RenderOutput::Surface { surface, .. } => {
                let surface_texture = surface.get_current_texture()?;
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                (view, Some(surface_texture))
            }
            RenderOutput::Buffer { texture, .. } => {
                (texture.create_view(&Default::default()), None)
            }
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.hdr.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            render_pass.set_pipeline(&self.render_pipeline);
            for (model, range) in &pre_frame_data.objects {
                render_pass.draw_model_instanced(
                    model,
                    range.clone(),
                    &self.camera_bind_group,
                    &self.light_bind_group,
                );
            }

            render_pass.set_pipeline(&self.light_render_pipeline);
            for (model, range) in &pre_frame_data.lights {
                render_pass.draw_light_model_instanced(
                    model,
                    range.clone(),
                    &self.camera_bind_group,
                    &self.light_bind_group,
                );
            }
        }

        self.hdr.process(&mut encoder, &view);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Debug"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.debug.draw_axis(&mut pass, &self.camera_bind_group);
        }

        match &mut self.output {
            RenderOutput::Surface { .. } => {
                self.queue.submit(std::iter::once(encoder.finish()));

                if let Some(texture) = surface_texture {
                    texture.present();
                }
            }
            RenderOutput::Buffer {
                width,
                height,
                padded_bytes_per_row,
                texture,
                buffer,
            } => {
                encoder.copy_texture_to_buffer(
                    wgpu::ImageCopyTexture {
                        aspect: wgpu::TextureAspect::All,
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                    },
                    wgpu::ImageCopyBuffer {
                        buffer,
                        layout: wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(*padded_bytes_per_row),
                            rows_per_image: None,
                        },
                    },
                    texture.size(),
                );

                self.queue.submit(std::iter::once(encoder.finish()));
            }
        }

        Ok(())
    }

    pub fn window(&self) -> Option<&Window> {
        match &self.output {
            RenderOutput::Surface { window, .. } => Some(window),
            RenderOutput::Buffer { .. } => None,
        }
    }

    pub async fn image_buffer(&self) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        match &self.output {
            RenderOutput::Buffer {
                width,
                height,
                padded_bytes_per_row,
                buffer,
                ..
            } => {
                let image_buffer = {
                    let buffer_slice = buffer.slice(..);

                    let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
                    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                        tx.send(result).unwrap();
                    });
                    self.device.poll(wgpu::Maintain::Wait);
                    rx.receive().await.unwrap().unwrap();

                    let padded_data = buffer_slice.get_mapped_range();

                    let mut pixels = Vec::with_capacity((*width * *height * 4) as usize);

                    for row in 0..*height {
                        let start = (row * *padded_bytes_per_row) as usize;
                        let end = start + (*width * 4) as usize;
                        pixels.extend_from_slice(&padded_data[start..end]);
                    }

                    ImageBuffer::<Rgba<u8>, _>::from_raw(*width, *height, pixels).unwrap()
                };

                buffer.unmap();

                Some(image_buffer)
            }
            RenderOutput::Surface { .. } => None,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        match &self.output {
            RenderOutput::Surface { window, .. } => window.inner_size().into(),
            RenderOutput::Buffer { width, height, .. } => (*width, *height),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }
}
