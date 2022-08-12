use crate::renderer::Renderer;
use crate::simple_render_system::Vertex;

use vulkano::DeviceSize;
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, DeviceLocalBuffer, BufferContents};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBuffer};
use vulkano::sync::GpuFuture;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use nalgebra::{Matrix4, Vector3};

#[derive(Clone)]
pub struct Transform3D {
    pub translation: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub rotation: Vector3<f32>,
}

impl Transform3D {
    pub fn mat4(&self) -> Matrix4<f32> {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();

        Matrix4::from_column_slice(&[
            self.scale.x * (c1 * c3 + s1 * s2 * s3),
            self.scale.x * (c2 * s3),
            self.scale.x * (c1 * s2 * s3 - c3 * s1),
            0.0,

            self.scale.y * (c3 * s1 * s2 - c1 * s3),
            self.scale.y * (c2 * c3),
            self.scale.y * (c1 * c3 * s2 + s1 * s3),
            0.0,

            self.scale.z * (c2 * s1),
            self.scale.z * (-s2),
            self.scale.z * (c1 * c2),
            0.0,

            self.translation.x, 
            self.translation.y, 
            self.translation.z, 
            1.0,
        ])
    }
}

static COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct GameObject {
    pub id: u32,
    pub transform: Transform3D,
    pub color: [f32; 3],
    pub model: Option<Arc<Model>>,
}

impl GameObject {
    pub fn new(model: Option<Arc<Model>>) -> GameObject {
        let id = COUNT.load(Ordering::SeqCst);
        COUNT.fetch_add(1, Ordering::SeqCst);

        let transform = Transform3D {
            translation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
        };

        GameObject {
            id,
            transform,
            color: [0.0, 0.0, 0.0],
            model,
        }
    }
}

pub struct Model {
    pub vertex_buffer: Arc<DeviceLocalBuffer<[Vertex]>>,
    pub normals_buffer: Option<Arc<DeviceLocalBuffer<[Vertex]>>>,
    pub index_buffer: Option<Arc<DeviceLocalBuffer<[u32]>>>,
}

impl Model {
    pub fn new(renderer: &Renderer, vertices: Vec<Vertex>, indices: Option<Vec<u32>>, normals: Option<Vec<Vertex>>) -> Self {

        let vertex_buffer = Model::create_device_local_buffer(renderer, vertices, BufferUsage::vertex_buffer());

        let normals_buffer = if normals.is_some() { Some( 
            Model::create_device_local_buffer(renderer, normals.unwrap(), BufferUsage::all())
        )} else {
            None
        };
        let index_buffer = if indices.is_some() { Some( 
            Model::create_device_local_buffer(renderer, indices.unwrap(), BufferUsage::index_buffer())
        )} else {
            None
        };

        Self {
            vertex_buffer,
            normals_buffer,
            index_buffer,
        }
    }

    fn create_device_local_buffer<T>(
        renderer: &Renderer, 
        data: Vec<T>, 
        usage: BufferUsage, 
    ) -> Arc<DeviceLocalBuffer<[T]>>
    where 
        [T]: BufferContents
    {
        let buffer_length = data.len() as u64;
        let staging_buffer = 
            CpuAccessibleBuffer::from_iter(
                renderer.device.clone(), 
                BufferUsage::transfer_src(), 
                false, 
                data
            ).expect("Failed to create vertex staging buffer");
        let device_local_buffer = DeviceLocalBuffer::<[T]>::array(
            renderer.device.clone(), 
            buffer_length as DeviceSize,
            usage | BufferUsage::transfer_dst(),
            renderer.device.active_queue_families(),
        ).expect("Failed to allocate vertex buffer");

        let mut cbb = AutoCommandBufferBuilder::primary(
            renderer.device.clone(),
            renderer.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        ).expect("Failed to create command buffer builder");

        cbb.copy_buffer(CopyBufferInfo::buffers(
            staging_buffer,
            device_local_buffer.clone(),
        )).expect("Failed to copy vertex buffer");
        let cb = cbb.build().expect("Failed to build command buffer");

        cb.execute(renderer.queue.clone()).unwrap()
            .then_signal_fence_and_flush().unwrap()
            .wait(None).unwrap();

        device_local_buffer
    }
}