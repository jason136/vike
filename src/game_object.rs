use crate::app::*;

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
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use nalgebra::{Matrix2, Vector2};

#[derive(Clone)]
pub struct Transform2D {
    pub translation: Vector2<f32>,
    pub scale: Vector2<f32>,
    pub rotation: f32,
}

impl Transform2D {
    pub fn mat2(&self) -> Matrix2<f32> {
        let s = self.rotation.sin();
        let c = self.rotation.cos();

        let rotation_mat = Matrix2::new(c, -s, s, c);
        let scale_mat = Matrix2::new(self.scale.x, 0.0, 0.0, self.scale.y);

        rotation_mat * scale_mat
    }
}

static COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct GameObject {
    pub id: u32,
    pub transform2d: Transform2D,
    pub color: [f32; 3],
    pub model: Option<Arc<CpuAccessibleBuffer<[Vertex]>>>,
}

impl GameObject {
    pub fn new() -> GameObject {
        let id = COUNT.load(Ordering::SeqCst);
        COUNT.fetch_add(1, Ordering::SeqCst);

        let transform2d = Transform2D {
            translation: Vector2::new(0.0, 0.0),
            scale: Vector2::new(1.0, 1.0),
            rotation: 0.0,
        };

        GameObject {
            id,
            transform2d,
            color: [0.0, 0.0, 0.0],
            model: None,
        }
    }
}