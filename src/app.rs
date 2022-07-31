use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, RenderingAttachmentInfo, RenderingInfo,
    },
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, QueueCreateInfo, Queue, 
    },
    image::{view::ImageView, ImageAccess, ImageUsage, SwapchainImage},
    impl_vertex,
    instance::{Instance, InstanceCreateInfo},
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            render_pass::PipelineRenderingCreateInfo,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, PipelineLayout, layout::PipelineLayoutCreateInfo,
    },
    render_pass::{LoadOp, StoreOp},
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError, Surface, 
    },
    sync::{self, FlushError, GpuFuture},
    Version,
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
                p.api_version() >= Version::V1_3
            })
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
    fn create_pipeline(device: Arc<Device>, swapchain: Arc<Swapchain<Window>>) -> Arc<GraphicsPipeline> {
        let vs = vs::load(device.clone()).expect("Failed to create vertex shader module");
        let fs = fs::load(device.clone()).expect("Failed to create fragment shader module");

        let pipeline_layout = PipelineLayout::new(device.clone(), PipelineLayoutCreateInfo{
            set_layouts: vec![],
            ..Default::default()
        }).expect("Failed to create pipeline layout");

        let pipeline = GraphicsPipeline::start()
            .render_pass(PipelineRenderingCreateInfo {
                color_attachment_formats: vec![Some(swapchain.image_format())], 
                ..Default::default()
            })
            //.vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .input_assembly_state(InputAssemblyState::new())
            .vertex_shader(vs.entry_point("main").expect("Failed to set vertex shader"), ())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").expect("Failed to set fragment shader"), ())
            .with_pipeline_layout(device.clone(), pipeline_layout).unwrap();
        pipeline
    }

    pub fn new() -> Self {
        let instance = FkApp::create_instance();
        let (event_loop, surface) = FkApp::create_window(instance.clone(), "Fraktal", 800, 600);
        let (device, queue) = FkApp::create_device(instance.clone(), surface.clone());
        let (swapchain, images) = FkApp::create_swapchain(surface.clone(), device.clone());
        let pipeline = FkApp::create_pipeline(device, swapchain);

        Self { 
            event_loop,
            surface, 
        }
    }

    pub fn main_loop(mut self) {
        let mut recreate_swapchain = false;

        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, 
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                // Event::WindowEvent { 
                //     event: WindowEvent::Resized(_), 
                //     ..
                // } => {
                //     recreate_swapchain = true;
                // }
                _ => (),
            }
        });
    }
}