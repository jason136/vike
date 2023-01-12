use crate::{
    renderer::Renderer,
    game_object::{Vertex, GameObject},
    camera::Camera,
};

use std::{sync::{Arc, Mutex}, collections::HashMap};
use std::collections::BTreeMap;
use vulkano::{
    buffer::TypedBufferAccess,
    command_buffer::{
        PrimaryAutoCommandBuffer, AutoCommandBufferBuilder
    },
    image::SampleCount,
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology}, 
            rasterization::{RasterizationState, PolygonMode, CullMode, FrontFace},
            multisample::MultisampleState,
            color_blend::ColorBlendState,
            depth_stencil::{DepthStencilState, DepthState, CompareOp},
            vertex_input::BuffersDefinition,
            viewport::{ViewportState, Viewport},
        },
        layout::{PushConstantRange, PipelineLayoutCreateInfo},
        GraphicsPipeline, StateMode, PartialStateMode, Pipeline, PipelineLayout, PipelineBindPoint,
    }, render_pass::Subpass,
};

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/billboard.vert",
        types_meta: {
            use bytemuck::{Pod, Zeroable};

            #[derive(Clone, Copy, Zeroable, Pod)]
        },
    }
}

pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/billboard.frag"
    }
}

pub struct PointLightSystem {
    pub pipeline: Arc<GraphicsPipeline>,
}

impl PointLightSystem {
    pub fn new(renderer: &Renderer) -> PointLightSystem {
        let pipeline = PointLightSystem::create_pipeline(renderer);

        PointLightSystem {
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

        let dimensions = renderer.surface.window().inner_size();
        let viewport_state = ViewportState::viewport_fixed_scissor_irrelevant([
            Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions.width as f32, dimensions.height as f32],
                depth_range: 0.0..1.0,
            },
        ]);

        let depth_stencil_state = DepthStencilState {
            depth: Some(DepthState{
                enable_dynamic: false, 
                write_enable: StateMode::Fixed(true),
                compare_op: StateMode::Fixed(CompareOp::Less),
            }),
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

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").expect("Failed to set vertex shader"), ())
            .input_assembly_state(input_assembly_state)
            .viewport_state(viewport_state)
            .fragment_shader(fs.entry_point("main").expect("Failed to set fragment shader"), ())
            .depth_stencil_state(depth_stencil_state)
            .render_pass(Subpass::from(renderer.render_pass.clone(), 0).unwrap())
            
            .rasterization_state(rasterization_state)
            .multisample_state(multisample_state)
            .color_blend_state(color_blend_state)

            .build(renderer.device.clone())
            .expect("Failed to create graphics pipeline");

        pipeline
    }

    pub fn render(
        &self,
        mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, 
    ) -> AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {

        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .draw(6, 1, 0, 0).unwrap();

        builder
    }
}