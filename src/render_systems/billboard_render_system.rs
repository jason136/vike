use crate::{
    camera::Camera,
    game_object::{GameObject, Vertex},
    renderer::Renderer,
};

use nalgebra::Vector3;
use std::{collections::HashMap, sync::Arc};
use vulkano::{
    buffer::cpu_pool::CpuBufferPoolSubbuffer,
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    image::SampleCount,
    memory::pool::StdMemoryPool,
    pipeline::{
        graphics::{
            color_blend::{
                AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
                ColorComponents,
            },
            depth_stencil::{CompareOp, DepthState, DepthStencilState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::{CullMode, FrontFace, PolygonMode, RasterizationState},
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, PartialStateMode, Pipeline, PipelineBindPoint, StateMode,
    },
    render_pass::Subpass,
};

use crate::render_systems::standard_render_system;

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/billboard_shader.vert",
        types_meta: {
            use bytemuck::{Pod, Zeroable};

            #[derive(Clone, Copy, Zeroable, Pod)]
        },
    }
}

pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/billboard_shader.frag"
    }
}

const MAX_POINT_LIGHTS: usize = 10;

#[derive(Clone, Copy)]
pub struct PointLightUBO {
    pub position: [f32; 4],
    pub color: [f32; 4],
}

pub struct BillboardRenderSystem {
    pub pipeline: Arc<GraphicsPipeline>,
}

impl BillboardRenderSystem {
    pub fn new(renderer: &Renderer) -> BillboardRenderSystem {
        let pipeline = BillboardRenderSystem::create_pipeline(renderer);

        BillboardRenderSystem { pipeline }
    }

    pub fn create_pipeline(renderer: &Renderer) -> Arc<GraphicsPipeline> {
        let vs = vs::load(renderer.device.clone()).expect("Failed to create vertex shader module");
        let fs =
            fs::load(renderer.device.clone()).expect("Failed to create fragment shader module");

        let input_assembly_state = InputAssemblyState {
            topology: PartialStateMode::Fixed(PrimitiveTopology::TriangleList),
            ..Default::default()
        };

        let dimensions = renderer.surface.window().inner_size();
        let viewport_state = ViewportState::viewport_fixed_scissor_irrelevant([Viewport {
            origin: [0.0, 0.0],
            dimensions: [dimensions.width as f32, dimensions.height as f32],
            depth_range: 0.0..1.0,
        }]);

        let depth_stencil_state = DepthStencilState {
            depth: Some(DepthState {
                enable_dynamic: false,
                write_enable: StateMode::Fixed(true),
                compare_op: StateMode::Fixed(CompareOp::Less),
            }),
            ..Default::default()
        };

        let rasterization_state = RasterizationState {
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
            attachments: vec![ColorBlendAttachmentState {
                color_write_enable: StateMode::Fixed(true),
                color_write_mask: ColorComponents::all(),
                blend: Some(AttachmentBlend {
                    color_op: BlendOp::Add,
                    color_source: BlendFactor::SrcAlpha,
                    color_destination: BlendFactor::OneMinusSrcAlpha,
                    alpha_op: BlendOp::Add,
                    alpha_source: BlendFactor::One,
                    alpha_destination: BlendFactor::Zero,
                }),
            }],
            logic_op: None,
            ..Default::default()
        };

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(
                vs.entry_point("main").expect("Failed to set vertex shader"),
                (),
            )
            .input_assembly_state(input_assembly_state)
            .viewport_state(viewport_state)
            .fragment_shader(
                fs.entry_point("main")
                    .expect("Failed to set fragment shader"),
                (),
            )
            .depth_stencil_state(depth_stencil_state)
            .render_pass(Subpass::from(renderer.render_pass.clone(), 0).unwrap())
            .rasterization_state(rasterization_state)
            .multisample_state(multisample_state)
            .color_blend_state(color_blend_state)
            .build(renderer.device.clone())
            .expect("Failed to create graphics pipeline");

        pipeline
    }

    pub fn update_point_lights(
        &self,
        game_objects: &HashMap<u32, GameObject>,
    ) -> (
        i32,
        [standard_render_system::vs::ty::PointLight; MAX_POINT_LIGHTS],
    ) {
        let mut point_lights = [standard_render_system::vs::ty::PointLight {
            position: [0.0, 0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        }; MAX_POINT_LIGHTS];

        let mut light_index: usize = 0;
        for obj in game_objects.values() {
            if let Some(light) = obj.point_light {
                point_lights[light_index] = standard_render_system::vs::ty::PointLight {
                    position: [
                        obj.transform.translation.x,
                        obj.transform.translation.y,
                        obj.transform.translation.z,
                        1.0,
                    ],
                    color: [obj.color.x, obj.color.y, obj.color.z, light.light_intensity],
                };
                light_index += 1;
            }
        }

        (light_index as i32, point_lights)
    }

    pub fn render(
        &self,
        mut builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        uniform_buffer_subbuffer: Arc<
            CpuBufferPoolSubbuffer<
                standard_render_system::vs::ty::UniformBufferData,
                Arc<StdMemoryPool>,
            >,
        >,
        game_objects: &HashMap<u32, GameObject>,
        camera: &Camera,
    ) -> AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        let mut sorted: Vec<(u32, u32)> = Vec::new();
        for obj in game_objects.values() {
            if obj.point_light.is_none() {
                continue;
            }

            let offset = Vector3::new(
                camera.inverse_view_matrix.data.0[3][0],
                camera.inverse_view_matrix.data.0[3][1],
                camera.inverse_view_matrix.data.0[3][2],
            ) - obj.transform.translation;
            let dist_squared = offset.dot(&offset);
            sorted.push(((dist_squared * 1000000000.0) as u32, obj.id));
        }
        sorted.sort_by(|&(a, _), &(b, _)| b.cmp(&a));

        let layout = self.pipeline.layout().set_layouts().get(0).unwrap();
        let set = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
        )
        .unwrap();
        builder.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            self.pipeline.layout().clone(),
            0,
            set.clone(),
        );

        for (_, id) in sorted.into_iter() {
            let obj = game_objects.get(&id).unwrap();

            let light = obj.point_light.unwrap();

            let push_constants = vs::ty::PushConstantData {
                position: [
                    obj.transform.translation.x,
                    obj.transform.translation.y,
                    obj.transform.translation.z,
                    1.0,
                ],
                color: [obj.color.x, obj.color.y, obj.color.z, light.light_intensity],
                radius: obj.transform.scale.x,
            };

            builder
                .bind_pipeline_graphics(self.pipeline.clone())
                .push_constants(self.pipeline.layout().clone(), 0, push_constants)
                .draw(6, 1, 0, 0)
                .unwrap();
        }

        builder
    }
}
