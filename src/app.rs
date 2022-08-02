use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        PrimaryAutoCommandBuffer, AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
    },
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, QueueCreateInfo, Queue, 
    },
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
        layout::PipelineLayoutCreateInfo,
        GraphicsPipeline, PipelineLayout, StateMode, PartialStateMode,
    },
    render_pass::{RenderPass, LoadOp, StoreOp, Subpass, Framebuffer, FramebufferCreateInfo},
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError, Surface, self, 
    },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, self},
    window::{Window, WindowBuilder},
    dpi::LogicalSize
};

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

pub struct FkApp {
    event_loop: EventLoop<()>,
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    pipeline: Arc<GraphicsPipeline>,
    render_pass: Arc<RenderPass>,
    viewport: Viewport,
    framebuffers: Vec<Arc<Framebuffer>>,
    //command_buffer: PrimaryAutoCommandBuffer,
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
            .with_resizable(false)
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
            let image_format = Some(
                device.physical_device()
                    .surface_formats(&surface, Default::default())
                    .unwrap()[0].0,
            );

            Swapchain::new(
                device.clone(), 
                surface.clone(), 
                SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,
                    image_format,
                    image_extent: surface.window().inner_size().into(),
                    image_usage: ImageUsage::color_attachment(), 
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .iter()
                        .next()
                        .unwrap(),
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

    fn create_pipeline(device: Arc<Device>, swapchain: Arc<Swapchain<Window>>, render_pass: Arc<RenderPass>) -> Arc<GraphicsPipeline> {
        let vs = vs::load(device.clone()).expect("Failed to create vertex shader module");
        let fs = fs::load(device.clone()).expect("Failed to create fragment shader module");

        let input_assembly_state = InputAssemblyState {
            topology: PartialStateMode::Fixed(PrimitiveTopology::TriangleList),
            ..Default::default()
        };
        
        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [
                swapchain.surface().window().inner_size().width as f32, 
                swapchain.surface().window().inner_size().height as f32
            ],
            depth_range: 0.0..1.0,
        };
        let scissor = Scissor {
            origin: [0, 0],
            dimensions: [
                swapchain.surface().window().inner_size().width as u32, 
                swapchain.surface().window().inner_size().height as u32
            ],
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
                enable_dynamic: false, 
                write_enable: StateMode::Fixed(true),
                compare_op: StateMode::Fixed(CompareOp::Less),
            }),
            ..Default::default()
        };

        let pipeline_layout = PipelineLayout::new(
            device.clone(), 
            PipelineLayoutCreateInfo{
                set_layouts: vec![],
                push_constant_ranges: vec![],
                ..Default::default()
            }
        ).expect("Failed to create pipeline layout");

        let pipeline = GraphicsPipeline::start()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .input_assembly_state(input_assembly_state)
            .viewport_state(ViewportState::viewport_fixed_scissor_fixed(vec![(viewport, scissor)]))
            .rasterization_state(rasterization_state)
            .multisample_state(multisample_state)
            .color_blend_state(color_blend_state)
            .depth_stencil_state(depth_stencil_state)
            .vertex_shader(vs.entry_point("main").expect("Failed to set vertex shader"), ())
            .fragment_shader(fs.entry_point("main").expect("Failed to set fragment shader"), ())
            .with_pipeline_layout(device.clone(), pipeline_layout)
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
            .iter()
            .map(|image| {
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
        framebuffers: Vec<Arc<Framebuffer>>,
    ) -> PrimaryAutoCommandBuffer {
        let mut builder = AutoCommandBufferBuilder::primary(
            device.clone(), 
            queue.family(), 
            CommandBufferUsage::MultipleSubmit,
        ).unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    render_area_offset: [0, 0],
                    render_area_extent: swapchain.image_extent(),
                    clear_values: vec![Some([0.1, 0.1, 0.1, 1.0].into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffers[0].clone())
                },
                SubpassContents::Inline,
            ).unwrap()
            .bind_pipeline_graphics(pipeline.clone())
            .draw(3, 1, 0, 0).unwrap()
            .end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        command_buffer
    }

    fn draw_frame(swapchain: Arc<Swapchain<Window>>, command_buffer: PrimaryAutoCommandBuffer, device: Arc<Device>, queue: Arc<Queue>) {
        let (image_index, suboptimal, acquire_future) = 
        match acquire_next_image(swapchain.clone(), None) {
            Ok(r) => r, 
            Err(AcquireError::OutOfDate) => {
                // recreate_swapchain = true;
                return;
            }
            Err(e) => panic!("Failed to acquire next image: {:?}", e),
        };

        let fence_signal = acquire_future
            .then_execute(queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_index)
            .then_signal_fence_and_flush().unwrap(); 

        fence_signal.wait(None).unwrap();
    }

    pub fn new() -> Self {
        let instance = FkApp::create_instance();
        let (event_loop, surface) = FkApp::create_window(instance.clone(), "Fraktal", 800, 600);
        let (device, queue) = FkApp::create_device(instance.clone(), surface.clone());
        let (swapchain, images) = FkApp::create_swapchain(surface.clone(), device.clone());
        let render_pass = FkApp::create_render_pass(device.clone(), swapchain.clone());
        let pipeline = FkApp::create_pipeline(device.clone(), swapchain.clone(), render_pass.clone());
        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };
        let framebuffers = FkApp::create_framebuffers(&images, render_pass.clone(), &mut viewport);
        let command_buffer = FkApp::create_command_buffers(device.clone(), queue.clone(), swapchain.clone(), pipeline.clone(), framebuffers.clone());

        FkApp::draw_frame(swapchain.clone(), command_buffer, device.clone(), queue.clone());

        Self { 
            event_loop,
            surface,
            device,
            images, 
            queue, 
            swapchain, 
            pipeline,
            render_pass,
            viewport, 
            framebuffers, 
            //command_buffer,
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

                    // let dimensions = self.surface.window().inner_size();
                    // if dimensions.width == 0 || dimensions.height == 0 {
                    //     return;
                    // }
                    // previous_frame_end.as_mut().unwrap().cleanup_finished();

                    // if recreate_swapchain {
                    //     let (new_swapchain, new_images) = 
                    //         match self.swapchain.recreate(SwapchainCreateInfo {
                    //             image_extent: dimensions.into(), 
                    //             ..self.swapchain.create_info()
                    //         }) {
                    //             Ok(r) => r, 
                    //             Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                    //             Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                    //         };

                    //     self.swapchain = new_swapchain;
                    //     self.framebuffers = FkApp::create_framebuffers(&new_images, self.render_pass.clone(), &mut self.viewport);
                    //     recreate_swapchain = false;
                    // }

                    // let (image_num, suboptimal, acquire_future) = 
                    //     match acquire_next_image(self.swapchain.clone(), None) {
                    //         Ok(r) => r, 
                    //         Err(AcquireError::OutOfDate) => {
                    //             recreate_swapchain = true;
                    //             return;
                    //         }
                    //         Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    //     };
                    // if suboptimal {
                    //     recreate_swapchain = true;
                    // }

                    // let future = previous_frame_end
                    //     .take().unwrap()
                    //     .join(acquire_future)
                    //     .then_execute(self.queue.clone(), command_buffer).unwrap()
                    //     .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
                    //     .then_signal_fence_and_flush();

                    // match future {
                    //     Ok(future) => {
                    //         previous_frame_end = Some(future.boxed());
                    //     }
                    //     Err(FlushError::OutOfDate) => {
                    //         recreate_swapchain = true;
                    //         previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                    //     }
                    //     Err(e) => {
                    //         println!("Failed to flush future: {:?}", e);
                    //         previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                    //     }
                    // }
                }
                _ => (),
            }
        });
    }
}