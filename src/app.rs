use crate::game_object::{self, GameObject};

use bytemuck::{Pod, Zeroable};
use std::sync::{Arc, Mutex};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        PrimaryAutoCommandBuffer, AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
    },
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, QueueCreateInfo, Queue, 
    },
    format::Format,
    image::{view::ImageView, ImageAccess, ImageUsage, SwapchainImage, SampleCount},
    impl_vertex,
    instance::{Instance, InstanceCreateInfo},
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology, }, 
            render_pass::PipelineRenderingCreateInfo,
            rasterization::{RasterizationState, PolygonMode, CullMode, FrontFace},
            multisample::{MultisampleState},
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{DepthStencilState, DepthState, CompareOp},
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState, Scissor},
        },
        layout::{PipelineLayoutCreateInfo, PushConstantRange},
        GraphicsPipeline, PipelineLayout, StateMode, PartialStateMode, Pipeline,
    },
    render_pass::{RenderPass, LoadOp, StoreOp, Subpass, Framebuffer, FramebufferCreateInfo},
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError, Surface, self, ColorSpace, PresentMode,
    },
    shader::{ShaderStages, },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, self},
    window::{Window, WindowBuilder},
    dpi::LogicalSize
};
use nalgebra::{Matrix2};

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/simple_shader.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/simple_shader.frag"
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}
impl_vertex!(Vertex, position, color);

pub struct FkApp {
    game_objects: Arc<Mutex<Vec<game_object::GameObject>>>,
    event_loop: EventLoop<()>,
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    pipeline: Arc<GraphicsPipeline>,
    render_pass: Arc<RenderPass>,
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    viewport: Viewport,
    framebuffers: Vec<Arc<Framebuffer>>,
}

impl FkApp {
    fn create_instance() -> Arc<Instance> {
        let required_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(InstanceCreateInfo { 
            enabled_extensions: required_extensions,
            enumerate_portability: true,
            ..Default::default()
        }).expect("Failed to create instance");
        instance
    }

    fn create_window(instance: Arc<Instance>, title: &str, width: u32, height: u32) -> (EventLoop<()>, Arc<Surface<Window>>) {
        let event_loop = EventLoop::new();
        let surface = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width as f64, height as f64))
            .with_resizable(true)
            .build_vk_surface(&event_loop, instance.clone())
            .expect("Failed to create surface");
        (event_loop, surface)
    }

    fn create_device(instance: Arc<Instance>, surface: Arc<Surface<Window>>) -> (Arc<Device>, Arc<Queue>) {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true, 
            ..DeviceExtensions::none()
        };

        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| {
                p.supported_extensions().is_superset_of(&device_extensions)
            })
            .filter_map(|p| {
                p.queue_families().find(|&q| {
                    q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false)
                })
                .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| {
                match p.properties().device_type {
                    PhysicalDeviceType::DiscreteGpu => 0,
                    PhysicalDeviceType::IntegratedGpu => 1,
                    PhysicalDeviceType::VirtualGpu => 2,
                    PhysicalDeviceType::Cpu => 3,
                    PhysicalDeviceType::Other => 4,
                }
            }).expect("No suitable physical device found.");

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type
        );

        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                enabled_extensions: device_extensions,
                enabled_features: Features {
                    dynamic_rendering: true, 
                    ..Features::none()
                },
                queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
                ..Default::default()
            },
        ).expect("Failed to create logical device");
        let queue = queues.next().unwrap();
        (device, queue)
    }

    fn create_swapchain(
        surface: Arc<Surface<Window>>, 
        device: Arc<Device>
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let (swapchain, images) = {
            let surface_capabilities = device.physical_device()
                .surface_capabilities(&surface, Default::default())
                .expect("Failed to get surface capabilities");
            let available_image_formats = Some(
                device.physical_device()
                    .surface_formats(&surface, Default::default())
                    .expect("Failed to get available image formats"),
            ).unwrap();
            let mut image_format = (Format::B8G8R8A8_SRGB, ColorSpace::SrgbNonLinear);
            if !available_image_formats.contains(&image_format) {
                image_format = available_image_formats[0];
            }

            Swapchain::new(
                device.clone(), 
                surface.clone(), 
                SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,
                    image_format: Some(image_format.0),
                    image_color_space: image_format.1,
                    image_extent: surface.window().inner_size().into(),
                    image_usage: ImageUsage::color_attachment(), 
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha.iter().next().unwrap(),
                    present_mode: PresentMode::Fifo,
                    ..Default::default()
                }
            ).expect("Failed to create swapchain")
        };

        (swapchain, images)
    }

    fn create_render_pass(device: Arc<Device>, swapchain: Arc<Swapchain<Window>>) -> Arc<RenderPass> {
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        ).unwrap();
        render_pass
    }

    fn create_pipeline(
        device: Arc<Device>, 
        render_pass: Arc<RenderPass>
    ) -> Arc<GraphicsPipeline> {
        let vs = vs::load(device.clone()).expect("Failed to create vertex shader module");
        let fs = fs::load(device.clone()).expect("Failed to create fragment shader module");

        let input_assembly_state = InputAssemblyState {
            topology: PartialStateMode::Fixed(PrimitiveTopology::TriangleList),
            ..Default::default()
        };

        let rasterization_state = RasterizationState{ 
            depth_clamp_enable: false,
            rasterizer_discard_enable: StateMode::Fixed(false),
            polygon_mode: PolygonMode::Fill,
            line_width: StateMode::Fixed(1.0),
            cull_mode: StateMode::Fixed(CullMode::None),
            front_face: StateMode::Fixed(FrontFace::Clockwise),
            depth_bias: None,
            ..Default::default() 
        };

        let multisample_state = MultisampleState {
            rasterization_samples: SampleCount::Sample1,
            sample_shading: None,
            ..Default::default()
        };

        let color_blend_state = ColorBlendState {
            logic_op: None, 
            ..Default::default()
        };

        let depth_stencil_state = DepthStencilState {
            depth: Some(DepthState{
                enable_dynamic: true, 
                write_enable: StateMode::Fixed(true),
                compare_op: StateMode::Fixed(CompareOp::Less),
            }),
            ..Default::default()
        };

        let push_constant_range = PushConstantRange {
            stages: ShaderStages {
                vertex: true, 
                fragment: true,
                ..Default::default()
            },
            offset: 0,
            size: std::mem::size_of::<vs::ty::PushConstantData>() as u32,
        };
        let pipeline_layout = PipelineLayout::new(
            device.clone(), 
            PipelineLayoutCreateInfo{
                set_layouts: vec![],
                push_constant_ranges: vec![push_constant_range],
                ..Default::default()
            }
        ).expect("Failed to create pipeline layout");

        let pipeline = GraphicsPipeline::start()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .input_assembly_state(input_assembly_state)
            .rasterization_state(rasterization_state)
            .multisample_state(multisample_state)
            .color_blend_state(color_blend_state)
            .depth_stencil_state(depth_stencil_state)
            .vertex_shader(vs.entry_point("main").expect("Failed to set vertex shader"), ())
            .fragment_shader(fs.entry_point("main").expect("Failed to set fragment shader"), ())
            .with_pipeline_layout(device.clone(), pipeline_layout.clone())
            .expect("Failed to create graphics pipeline");
        
        pipeline
    }

    fn create_framebuffers(
        images: &[Arc<SwapchainImage<Window>>],
        render_pass: Arc<RenderPass>,
        viewport: &mut Viewport,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();
        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
    
        images
            .iter().map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    }

    fn create_command_buffers(
        device: Arc<Device>, 
        queue: Arc<Queue>, 
        swapchain: Arc<Swapchain<Window>>, 
        pipeline: Arc<GraphicsPipeline>, 
        viewport: &mut Viewport,
        framebuffer: Arc<Framebuffer>,
        game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
    ) -> PrimaryAutoCommandBuffer {
        let mut builder = AutoCommandBufferBuilder::primary(
            device.clone(), 
            queue.family(), 
            CommandBufferUsage::OneTimeSubmit,
        ).unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    render_area_offset: [0, 0],
                    render_area_extent: swapchain.image_extent(),
                    clear_values: vec![Some([0.01, 0.01, 0.01, 1.0].into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                SubpassContents::Inline,
            ).unwrap()
            .set_viewport(0, [viewport.clone()])
            .bind_pipeline_graphics(pipeline.clone());
            
        builder = FkApp::render_game_objects(builder, game_objects, pipeline);

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        command_buffer
    }

    fn render_game_objects(
        mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, 
        game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
        pipeline: Arc<GraphicsPipeline>, 
    ) -> AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        for obj in game_objects.lock().unwrap().iter().rev() {
            let push_constants = vs::ty::PushConstantData {
                transform: obj.transform2d.mat2().into(),
                offset: obj.transform2d.translation.into(),
                color: obj.color,
                _dummy0: [0, 0, 0, 0, 0, 0, 0, 0],
            };
            builder
                .bind_vertex_buffers(0, obj.model.clone().unwrap())
                .push_constants(pipeline.layout().clone(), 0, push_constants)
                .draw(obj.model.clone().unwrap().len() as u32, 1, 0, 0).unwrap();
        }

        builder
    }

    fn animate_game_objects(
        game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
    ) {
        let mut i = 1.0;
        for obj in game_objects.lock().unwrap().iter_mut() {
            obj.transform2d.rotation += 0.001 * std::f32::consts::PI * 2.0 * i;
            i += 1.0;
        }
    }

    fn create_game_objects(device: Arc<Device>) -> (Arc<CpuAccessibleBuffer<[Vertex]>>, Vec<game_object::GameObject>) {
        let vertices = vec![
            Vertex { position: [0.0, -0.5], color: [1.0, 0.0, 0.0] },
            Vertex { position: [0.5, 0.5], color: [0.0, 1.0, 0.0] },
            Vertex { position: [-0.5, 0.5], color: [0.0, 0.0, 1.0] },
        ];

        let vertex_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::vertex_buffer(), false, vertices)
            .expect("Failed to create vertex buffer");
        
        let mut game_objects = vec![];

        let mut colors: Vec<[f32; 3]> = vec![
            [1.0, 0.7, 0.73],
            [1.0, 0.87, 0.73],
            [1.0, 1.0, 0.73],
            [0.73, 1.0, 0.8],
            [0.73, 0.88, 1.0],
        ];
        for i in 0..5 {
            colors[i] = [colors[i][0].powf(2.2), colors[i][1].powf(2.2), colors[i][2].powf(2.2)];
        }

        for i in 0..40 {
            let mut triangle = GameObject::new();
            triangle.model = Some(vertex_buffer.clone());

            triangle.color = colors[i % colors.len()];
            triangle.transform2d.translation.x = 0.0;
            triangle.transform2d.scale = [0.5 + i as f32 * 0.025, 0.5 + i as f32 * 0.025].into();
            triangle.transform2d.rotation = i as f32 * 0.025 * std::f32::consts::PI;
            game_objects.push(triangle);
        };
        
        (vertex_buffer, game_objects)
    }

    pub fn new() -> Self {
        let instance = FkApp::create_instance();
        let (event_loop, surface) = FkApp::create_window(instance.clone(), "Fraktal", 800, 600);
        let (device, queue) = FkApp::create_device(instance.clone(), surface.clone());
        let (swapchain, images) = FkApp::create_swapchain(surface.clone(), device.clone());
        let render_pass = FkApp::create_render_pass(device.clone(), swapchain.clone());
        let pipeline = FkApp::create_pipeline(device.clone(), render_pass.clone());

        
        let (vertex_buffer, obj_vec) = FkApp::create_game_objects(device.clone());
        let game_objects = Arc::new(Mutex::new(obj_vec));

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };
        let framebuffers = FkApp::create_framebuffers(&images, render_pass.clone(), &mut viewport);

        Self { 
            game_objects,
            event_loop,
            surface,
            device,
            images, 
            queue, 
            swapchain, 
            pipeline,
            render_pass,
            vertex_buffer,
            viewport, 
            framebuffers, 
        }
    }

    pub fn main_loop(mut self) {
        let mut recreate_swapchain = false;
        let mut previous_frame_end = Some(sync::now(self.device.clone()).boxed());

        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, 
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent { 
                    event: WindowEvent::Resized(_), 
                    ..
                 } => {
                    recreate_swapchain = true;
                 }
                Event::RedrawEventsCleared => {
                    let dimensions = self.surface.window().inner_size();
                    if dimensions.width == 0 || dimensions.height == 0 {
                        return;
                    }

                    previous_frame_end.as_mut().unwrap().cleanup_finished();

                    if recreate_swapchain {
                        let (new_swapchain, new_images) =
                            match self.swapchain.recreate(SwapchainCreateInfo {
                                image_extent: dimensions.into(),
                                ..self.swapchain.create_info()
                            }) {
                                Ok(r) => r,
                                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                            };

                        self.swapchain = new_swapchain;
                        self.framebuffers = FkApp::create_framebuffers(
                            &new_images,
                            self.render_pass.clone(),
                            &mut self.viewport,
                        );
                        recreate_swapchain = false;
                    }

                    let (image_num, suboptimal, acquire_future) =
                        match acquire_next_image(self.swapchain.clone(), None) {
                            Ok(r) => r,
                            Err(AcquireError::OutOfDate) => {
                                recreate_swapchain = true;
                                return;
                            }
                            Err(e) => panic!("Failed to acquire next image: {:?}", e),
                        };
                    if suboptimal {
                        recreate_swapchain = true;
                    }

                    FkApp::animate_game_objects(self.game_objects.clone());

                    let command_buffer = FkApp::create_command_buffers(
                        self.device.clone(), 
                        self.queue.clone(), 
                        self.swapchain.clone(), 
                        self.pipeline.clone(), 
                        &mut self.viewport.clone(),
                        self.framebuffers[image_num].clone(), 
                        self.game_objects.clone(),
                    );
                    let future = previous_frame_end
                        .take().unwrap()
                        .join(acquire_future)
                        .then_execute(self.queue.clone(), command_buffer).unwrap()
                        .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
                        .then_signal_fence_and_flush();

                    match future {
                        Ok(future) => {
                            previous_frame_end = Some(future.boxed());
                        }
                        Err(FlushError::OutOfDate) => {
                            recreate_swapchain = true;
                            previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                        }
                        Err(e) => {
                            println!("Failed to flush future: {:?}", e);
                            previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                        }
                    }
                }
                _ => (),
            }
        });
    }
}