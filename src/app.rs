use crate::{
    simple_render_system::{SimpleRenderSystem, Vertex},
    renderer::Renderer,
    game_object::{self, GameObject},
};

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

fn animate_game_objects(
    game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
) {
    let mut i = 1.0;
    for obj in game_objects.lock().unwrap().iter_mut() {
        obj.transform2d.rotation += 0.001 * std::f32::consts::PI * 2.0 * i;
        i += 1.0;
    }
}

fn create_game_objects(renderer: &Renderer) -> Vec<game_object::GameObject> {
    let vertices = vec![
        Vertex { position: [0.0, -0.5], color: [1.0, 0.0, 0.0] },
        Vertex { position: [0.5, 0.5], color: [0.0, 1.0, 0.0] },
        Vertex { position: [-0.5, 0.5], color: [0.0, 0.0, 1.0] },
    ];

    let vertex_buffer = CpuAccessibleBuffer::from_iter(renderer.device.clone(), BufferUsage::vertex_buffer(), false, vertices)
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
        colors[i] = [colors[i][0].powf(4.0), colors[i][1].powf(4.0), colors[i][2].powf(4.0)];
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
    
    game_objects
}

pub struct FkApp {
    pub event_loop: EventLoop<()>,
    pub renderer: Renderer,
    pub simple_render_system: SimpleRenderSystem,
    pub game_objects: Arc<Mutex<Vec<game_object::GameObject>>>,
}

impl FkApp {
    pub fn new() -> Self {
        let (event_loop, surface, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, surface);
        let simple_render_system = SimpleRenderSystem::new(&renderer);

        let obj_vec = create_game_objects(&renderer);
        let game_objects = Arc::new(Mutex::new(obj_vec));

        Self {
            event_loop,
            renderer, 
            simple_render_system,
            game_objects,
        }
    }

    pub fn main_loop(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, 
                    ..
                } => {
                    println!("Window closed");
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent { 
                    event: WindowEvent::Resized(_), 
                    ..
                 } => {
                    println!("Window resized");
                    self.renderer.recreate_swapchain = true;
                 }
                Event::RedrawEventsCleared => {
                    
                    if let Some((mut builder, acquire_future)) = self.renderer.begin_frame(&self.simple_render_system) {

                        animate_game_objects(self.game_objects.clone());
                        builder = self.simple_render_system.render_game_objects(builder, self.game_objects.clone());

                        self.renderer.end_frame(builder, acquire_future);
                    }
                }
                _ => (),
            }
        });
    }
}