use std::{sync::Arc, time::Instant};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo,
    },
    format::Format,
    image::{view::ImageView, AttachmentImage, ImageAccess, ImageUsage, SwapchainImage},
    instance::{Instance, InstanceCreateInfo},
    pipeline::{
        graphics::{
            depth_stencil::DepthStencilState,
            input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::ShaderModule,
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError, Surface,
    },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

struct Fraktal {
    instance: Arc<Instance>, 
    event_loop: EventLoop<()>,
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    images: Vec<Arc<SwapchainImage<Window>>>, 
}

impl Fraktal {
    pub fn initialize() -> Self {
        let required_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(InstanceCreateInfo { 
            enabled_extensions: required_extensions,
            enumerate_portability: true,
            ..Default::default()
        }).unwrap();

        let event_loop = EventLoop::new();
        let surface = WindowBuilder::new()
            .build_vk_surface(&event_loop, instance.clone())
            .unwrap();

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
                queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
                ..Default::default()
            },
        ).unwrap();
        let queue = queues.next().unwrap();

        let (mut swapchain, images) = {
            let surface_capabilities = physical_device
                .surface_capabilities(&surface, Default::default())
                .expect("Failed to get surface capabilities");
            let image_format = Some(
                physical_device
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
            ).unwrap()
        };

        Self {
            instance,
            event_loop, 
            surface, 
            device, 
            queue, 
            swapchain, 
            images, 
        }
    }

    fn main_loop(&mut self) {
        let mut viewport = Viewport { origin: [0.0, 0.0], dimensions: [0.0, 0.0], depth_range: 0.0..1.0 };
        let mut framebuffers = Fraktal::window_size_dependent_setup(&self.images, self.render_pass.clone(), &mut viewport);
        let mut recreate_swapchain = false;
        let mut previous_fram_end = Some(sync::now(self.device.clone()).boxed());
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
                    previous_fram_end.as_mut().unwrap().cleanup_finished();

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
                        framebuffers = Fraktal::window_size_dependent_setup(
                            &new_images, 
                            self.render_pass.clone(), 
                            &mut viewport,
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

                    let mut builder = AutoCommandBufferBuilder::primary(
                        self.device.clone(), 
                        self.queue.family(), 
                        CommandBufferUsage::OneTimeSubmit,
                    ).unwrap();

                    builder.begin_render_pass(
                        RenderPassBeginInfo {
                            clear_values: vec![Some([0.0, 0.0, 1.0, 1.0].into())], 
                            ..RenderPassBeginInfo::framebuffer(framebuffers[image_num].clone())
                        }, 
                        SubpassContents::Inline,
                    ).unwrap()
                        .set_viewport(0, [viewport.clone()])
                        .bind_pipeline_graphics(pipeline.clone())
                        .draw(vertex_buffer.len() as u32, 1, 0, 0).unwrap()
                        .end_render_pass().unwrap();
                    
                    let command_buffer = builder.build().unwrap();

                    let future = previous_fram_end
                        .take().unwrap()
                        .join(acquire_future)
                        .then_execute(self.queue.clone(), command_buffer).unwrap()
                        .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
                        .then_signal_fence_and_flush();

                    match future {
                        Ok(future) => {
                            previous_fram_end = Some(future.boxed());
                        }
                        Err(FlushError::OutOfDate) => {
                            recreate_swapchain = true;
                            previous_fram_end = Some(sync::now(self.device.clone()).boxed());
                        }
                        Err(e) => {
                            println!("Failed to flush future: {:?}", e);
                            previous_fram_end = Some(sync::now(self.device.clone()).boxed());
                        }
                    }
                }
                _ => (),
            }
        });
    }

    fn window_size_dependent_setup(
        images: &[Arc<SwapchainImage<Window>>],
        render_pass: Arc<RenderPass>, 
        viewport: &mut Viewport,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();
        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

        images.iter().map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(), 
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            ).unwrap()
        }).collect::<Vec<_>>()
    }
}

fn main() {
    let mut fraktal = Fraktal::initialize();
    fraktal.main_loop();
}