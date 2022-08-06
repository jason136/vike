use crate::{
    renderer::Renderer,
    game_object::{self, GameObject}
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
    pub position: [f32; 2],
    pub color: [f32; 3],
}
impl_vertex!(Vertex, position, color);

pub struct SimpleRenderSystem {
    pub pipeline: Arc<GraphicsPipeline>,
}

impl SimpleRenderSystem {
    pub fn new(renderer: &Renderer) -> SimpleRenderSystem {
        let pipeline = SimpleRenderSystem::create_pipeline(renderer);

        SimpleRenderSystem {
            pipeline,
        }
    }

    pub fn create_pipeline(renderer: &Renderer) -> Arc<GraphicsPipeline> {
        let vs = vs::load(renderer.device.clone()).expect("Failed to create vertex shader module");
        let fs = fs::load(renderer.device.clone()).expect("Failed to create fragment shader module");

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
            renderer.device.clone(), 
            PipelineLayoutCreateInfo{
                set_layouts: vec![],
                push_constant_ranges: vec![push_constant_range],
                ..Default::default()
            }
        ).expect("Failed to create pipeline layout");

        let pipeline = GraphicsPipeline::start()
            .render_pass(Subpass::from(renderer.render_pass.clone(), 0).unwrap())
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .input_assembly_state(input_assembly_state)
            .rasterization_state(rasterization_state)
            .multisample_state(multisample_state)
            .color_blend_state(color_blend_state)
            .depth_stencil_state(depth_stencil_state)
            .vertex_shader(vs.entry_point("main").expect("Failed to set vertex shader"), ())
            .fragment_shader(fs.entry_point("main").expect("Failed to set fragment shader"), ())
            .with_pipeline_layout(renderer.device.clone(), pipeline_layout.clone())
            .expect("Failed to create graphics pipeline");
        
        pipeline
    }

    pub fn render_game_objects(
        &self,
        mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, 
        game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
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
                .push_constants(self.pipeline.layout().clone(), 0, push_constants)
                .draw(obj.model.clone().unwrap().len() as u32, 1, 0, 0).unwrap();
        }

        builder
    }
}