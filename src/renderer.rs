use crate::{
    simple_render_system::SimpleRenderSystem,
};

use std::sync::Arc;
use vulkano::{
    command_buffer::{
        PrimaryAutoCommandBuffer, AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
    },
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, QueueCreateInfo, Queue, 
    },
    format::Format,
    image::{view::ImageView, ImageAccess, ImageUsage, SwapchainImage, AttachmentImage},
    instance::{Instance, InstanceCreateInfo},
    pipeline::graphics::viewport::Viewport,
    render_pass::{RenderPass, Framebuffer, FramebufferCreateInfo},
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError, Surface, ColorSpace, PresentMode, SwapchainAcquireFuture,
    },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
    dpi::LogicalSize
};

pub struct Renderer {
    pub device: Arc<Device>,
    pub surface: Arc<Surface<Window>>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<Window>>,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub render_pass: Arc<RenderPass>,
    pub recreate_swapchain: bool,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    pub image_index: usize,
}

impl Renderer {
    pub fn new(instance: Arc<Instance>, surface: Arc<Surface<Window>>) -> Self {
        let (device, queue) = Renderer::create_device(instance.clone(), surface.clone());
        let (swapchain, images) = Renderer::create_swapchain(surface.clone(), device.clone());
        let render_pass = Renderer::create_render_pass(device.clone(), swapchain.clone());

        let framebuffers = Renderer::create_framebuffers(
            device.clone(),
            &images,
            render_pass.clone(),
        );

        let recreate_swapchain = false;
        let previous_frame_end = Some(sync::now(device.clone()).boxed());

        Renderer {
            device,
            surface,
            queue,
            swapchain,
            framebuffers,
            render_pass,
            recreate_swapchain,
            previous_frame_end,
            image_index: 0,
        }
    }

    pub fn create_window() -> (EventLoop<()>, Arc<Surface<Window>>, Arc<Instance>) {
        let instance = Renderer::create_instance();
        let (event_loop, surface) = Renderer::create_winit(instance.clone(), "Release", 800, 600);

        (event_loop, surface, instance)
    }

    pub fn begin_frame(&mut self, simple_render_system: &SimpleRenderSystem
    ) -> Option<(AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, SwapchainAcquireFuture<Window>, bool)> {
        let dimensions = self.surface.window().inner_size();
        if dimensions.width == 0 || dimensions.height == 0 {
            return None
        }

        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.recreate_swapchain {
            let (new_swapchain, new_images) =
                match self.swapchain.recreate(SwapchainCreateInfo {
                    image_extent: dimensions.into(),
                    ..self.swapchain.create_info()
                }) {
                    Ok(r) => r,
                    Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return None,
                    Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                };

            self.swapchain = new_swapchain;
            self.framebuffers = Renderer::create_framebuffers(
                self.device.clone(),
                &new_images,
                self.render_pass.clone(),
            );
        }

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return None
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        self.image_index = image_num;

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(), 
            self.queue.family(), 
            CommandBufferUsage::OneTimeSubmit,
        ).unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    render_area_offset: [0, 0],
                    render_area_extent: self.swapchain.image_extent(),
                    clear_values: vec![
                        Some([0.01, 0.01, 0.01, 1.0].into()),
                        Some(1f32.into()),
                    ],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffers[self.image_index].clone())
                },
                SubpassContents::Inline,
            ).unwrap()
            .bind_pipeline_graphics(simple_render_system.pipeline.clone());
            
        Some((builder, acquire_future, self.recreate_swapchain))
    }

    pub fn end_frame(&mut self, mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, acquire_future: SwapchainAcquireFuture<Window>) {
        self.recreate_swapchain = false;

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let future = self.previous_frame_end
            .take().unwrap()
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), self.image_index)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }

    fn create_instance() -> Arc<Instance> {
        let required_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(InstanceCreateInfo { 
            enabled_extensions: required_extensions,
            enumerate_portability: true,
            ..Default::default()
        }).expect("Failed to create instance");
        instance
    }

    fn create_winit(instance: Arc<Instance>, title: &str, width: u32, height: u32) -> (EventLoop<()>, Arc<Surface<Window>>) {
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
            image_format = available_image_formats[0];

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
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        ).unwrap();
        render_pass
    }

    fn create_framebuffers(
        device: Arc<Device>,
        images: &[Arc<SwapchainImage<Window>>],
        render_pass: Arc<RenderPass>,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();

        let depth_buffer = ImageView::new_default(
            AttachmentImage::transient(device.clone(), dimensions, Format::D16_UNORM).unwrap(),
        ).unwrap();
    
        images
            .iter().map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view, depth_buffer.clone()],
                        ..Default::default()
                    },
                ).unwrap()
            }).collect::<Vec<_>>()
    }
}