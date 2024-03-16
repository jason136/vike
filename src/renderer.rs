use nalgebra::{Normed, Quaternion, UnitQuaternion, UnitVector3, Vector3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{
    camera::{self, Camera, CameraUniform},
    game_object::{Instance, InstanceRaw, Transform3D, Vertex},
    texture::Texture,
};

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.00759614],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.43041354],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.949397],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.84732914],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.2652641],
    },
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

const NUM_INSTANCES_PER_ROW: u32 = 10;
const INSTANCE_DISPLACEMENT: Vector3<f32> = Vector3::new(
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    0.0,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
);

pub struct Renderer {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub diffuse_bind_group: wgpu::BindGroup,
    pub diffuse_texture: Texture,
    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
    pub depth_texture: Texture,
}

impl Renderer {
    pub async fn new(window: Window) -> Self {
        let size = window.inner_size();
        let window_arc = Arc::new(window);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window_arc.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
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

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let diffuse_bytes = include_bytes!("happy-tree.png");
        let diffuse_texture =
            Texture::from_bytes(&device, &queue, diffuse_bytes, "happy-tree.png").unwrap();

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
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/shader.wgsl"));

        let mut transform = Transform3D::new();
        transform.translation.z = -2.5;
        let mut camera = Camera::new(Some(transform));

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let position = Vector3::new(x as f32, 0.0, z as f32) - INSTANCE_DISPLACEMENT;

                    let rotation = if position == Vector3::zeros() {
                        UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 0.0)
                    } else {
                        UnitQuaternion::from_axis_angle(&UnitVector3::new_normalize(position), 45.0)
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), InstanceRaw::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

        Self {
            window: window_arc,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices: INDICES.len() as u32,
            diffuse_bind_group,
            diffuse_texture,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            instances,
            instance_buffer,
            depth_texture,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }

        self.depth_texture =
            Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..self.num_indices, 0, 0..self.instances.len() as _);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    // pub fn begin_frame(
    //     &mut self,
    // ) -> Option<(
    //     AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    //     SwapchainAcquireFuture<Window>,
    //     bool,
    // )> {
    //     let dimensions = self.surface.window().inner_size();
    //     if dimensions.width == 0 || dimensions.height == 0 {
    //         return None;
    //     }

    //     self.previous_frame_end.as_mut().unwrap().cleanup_finished();

    //     if self.recreate_swapchain {
    //         let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
    //             image_extent: dimensions.into(),
    //             ..self.swapchain.create_info()
    //         }) {
    //             Ok(r) => r,
    //             Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return None,
    //             Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
    //         };

    //         self.swapchain = new_swapchain;
    //         self.framebuffers = Renderer::create_framebuffers(
    //             self.device.clone(),
    //             &new_images,
    //             self.render_pass.clone(),
    //         );
    //     }

    //     let (image_num, suboptimal, acquire_future) =
    //         match acquire_next_image(self.swapchain.clone(), None) {
    //             Ok(r) => r,
    //             Err(AcquireError::OutOfDate) => {
    //                 self.recreate_swapchain = true;
    //                 return None;
    //             }
    //             Err(e) => panic!("Failed to acquire next image: {:?}", e),
    //         };
    //     if suboptimal {
    //         self.recreate_swapchain = true;
    //     }
    //     self.image_index = image_num;

    //     let mut builder = AutoCommandBufferBuilder::primary(
    //         self.device.clone(),
    //         self.queue.family(),
    //         CommandBufferUsage::OneTimeSubmit,
    //     )
    //     .unwrap();

    //     builder
    //         .begin_render_pass(
    //             RenderPassBeginInfo {
    //                 render_area_offset: [0, 0],
    //                 render_area_extent: self.swapchain.image_extent(),
    //                 clear_values: vec![Some([0.01, 0.01, 0.01, 1.0].into()), Some(1f32.into())],
    //                 ..RenderPassBeginInfo::framebuffer(self.framebuffers[self.image_index].clone())
    //             },
    //             SubpassContents::Inline,
    //         )
    //         .unwrap();

    //     Some((builder, acquire_future, self.recreate_swapchain))
    // }

    // pub fn end_frame(
    //     &mut self,
    //     mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    //     acquire_future: SwapchainAcquireFuture<Window>,
    // ) {
    //     self.recreate_swapchain = false;

    //     builder.end_render_pass().unwrap();
    //     let command_buffer = builder.build().unwrap();

    //     let future = self
    //         .previous_frame_end
    //         .take()
    //         .unwrap()
    //         .join(acquire_future)
    //         .then_execute(self.queue.clone(), command_buffer)
    //         .unwrap()
    //         .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), self.image_index)
    //         .then_signal_fence_and_flush();

    //     match future {
    //         Ok(future) => {
    //             self.previous_frame_end = Some(future.boxed());
    //         }
    //         Err(FlushError::OutOfDate) => {
    //             self.recreate_swapchain = true;
    //             self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
    //         }
    //         Err(e) => {
    //             println!("Failed to flush future: {:?}", e);
    //             self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
    //         }
    //     }
    // }

    // fn create_instance() -> Arc<Instance> {
    //     let required_extensions = vulkano_win::required_extensions();
    //     let instance = Instance::new(InstanceCreateInfo {
    //         enabled_extensions: required_extensions,
    //         enumerate_portability: true,
    //         ..Default::default()
    //     })
    //     .expect("Failed to create instance");

    //     instance
    // }

    // fn create_device(
    //     instance: Arc<Instance>,
    //     surface: Arc<Surface<Window>>,
    // ) -> (Arc<Device>, Arc<Queue>) {
    //     let device_extensions = DeviceExtensions {
    //         khr_swapchain: true,
    //         ..DeviceExtensions::none()
    //     };

    //     let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
    //         .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
    //         .filter_map(|p| {
    //             p.queue_families()
    //                 .find(|&q| {
    //                     q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false)
    //                 })
    //                 .map(|q| (p, q))
    //         })
    //         .min_by_key(|(p, _)| match p.properties().device_type {
    //             PhysicalDeviceType::DiscreteGpu => 0,
    //             PhysicalDeviceType::IntegratedGpu => 1,
    //             PhysicalDeviceType::VirtualGpu => 2,
    //             PhysicalDeviceType::Cpu => 3,
    //             PhysicalDeviceType::Other => 4,
    //         })
    //         .expect("No suitable physical device found.");

    //     // println!(
    //     //     "Using device: {} (type: {:?})",
    //     //     physical_device.properties().device_name,
    //     //     physical_device.properties().device_type
    //     // );

    //     let (device, mut queues) = Device::new(
    //         physical_device,
    //         DeviceCreateInfo {
    //             enabled_extensions: device_extensions,
    //             enabled_features: Features { ..Features::none() },
    //             queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
    //             ..Default::default()
    //         },
    //     )
    //     .expect("Failed to create logical device");
    //     let queue = queues.next().unwrap();

    //     (device, queue)
    // }

    // fn create_swapchain(
    //     surface: Arc<Surface<Window>>,
    //     device: Arc<Device>,
    // ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
    //     let (swapchain, images) = {
    //         let surface_capabilities = device
    //             .physical_device()
    //             .surface_capabilities(&surface, Default::default())
    //             .expect("Failed to get surface capabilities");
    //         let available_image_formats = Some(
    //             device
    //                 .physical_device()
    //                 .surface_formats(&surface, Default::default())
    //                 .expect("Failed to get available image formats"),
    //         )
    //         .unwrap();
    //         let mut image_format = (Format::B8G8R8A8_SRGB, ColorSpace::SrgbNonLinear);
    //         if !available_image_formats.contains(&image_format) {
    //             image_format = available_image_formats[0];
    //         }

    //         Swapchain::new(
    //             device.clone(),
    //             surface.clone(),
    //             SwapchainCreateInfo {
    //                 min_image_count: surface_capabilities.min_image_count,
    //                 image_format: Some(image_format.0),
    //                 image_color_space: image_format.1,
    //                 image_extent: surface.window().inner_size().into(),
    //                 image_usage: ImageUsage::color_attachment(),
    //                 composite_alpha: surface_capabilities
    //                     .supported_composite_alpha
    //                     .iter()
    //                     .next()
    //                     .unwrap(),
    //                 present_mode: PresentMode::Fifo,
    //                 ..Default::default()
    //             },
    //         )
    //         .expect("Failed to create swapchain")
    //     };

    //     (swapchain, images)
    // }

    // fn create_render_pass(
    //     device: Arc<Device>,
    //     swapchain: Arc<Swapchain<Window>>,
    // ) -> Arc<RenderPass> {
    //     let render_pass = vulkano::single_pass_renderpass!(
    //         device.clone(),
    //         attachments: {
    //             color: {
    //                 load: Clear,
    //                 store: Store,
    //                 format: swapchain.image_format(),
    //                 samples: 1,
    //             },
    //             depth: {
    //                 load: Clear,
    //                 store: DontCare,
    //                 format: Format::D16_UNORM,
    //                 samples: 1,
    //             }
    //         },
    //         pass: {
    //             color: [color],
    //             depth_stencil: {depth}
    //         }
    //     )
    //     .unwrap();

    //     render_pass
    // }

    // fn create_framebuffers(
    //     device: Arc<Device>,
    //     images: &[Arc<SwapchainImage<Window>>],
    //     render_pass: Arc<RenderPass>,
    // ) -> Vec<Arc<Framebuffer>> {
    //     let dimensions = images[0].dimensions().width_height();

    //     let depth_buffer = ImageView::new_default(
    //         AttachmentImage::transient(device.clone(), dimensions, Format::D16_UNORM).unwrap(),
    //     )
    //     .unwrap();

    //     let framebuffers = images
    //         .iter()
    //         .map(|image| {
    //             let view = ImageView::new_default(image.clone()).unwrap();
    //             Framebuffer::new(
    //                 render_pass.clone(),
    //                 FramebufferCreateInfo {
    //                     attachments: vec![view, depth_buffer.clone()],
    //                     ..Default::default()
    //                 },
    //             )
    //             .unwrap()
    //         })
    //         .collect::<Vec<_>>();

    //     framebuffers
    // }
}
