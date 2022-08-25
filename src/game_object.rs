use crate::renderer::Renderer;

use bytemuck::{Pod, Zeroable};
use tobj::load_obj;
use vulkano::{DeviceSize, impl_vertex};
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, DeviceLocalBuffer, BufferContents};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBuffer};
use vulkano::sync::GpuFuture;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use nalgebra::{Matrix4, Vector3, Matrix3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}
impl_vertex!(Vertex, position, color, normal, uv);

impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position &&
        self.color == other.color &&
        self.normal == other.normal &&
        self.uv == other.uv
    }
}
impl Eq for Vertex {}
impl Hash for Vertex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

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

    pub fn normal_matrix(&self) -> Matrix4<f32> {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();
        let inv_scale = Vector3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);

        Matrix4::from_column_slice(&[
            inv_scale.x * (c1 * c3 + s1 * s2 * s3),
            inv_scale.x * (c2 * s3),
            inv_scale.x * (c1 * s2 * s3 - c3 * s1),
            0.0, 

            inv_scale.y * (c3 * s1 * s2 - c1 * s3),
            inv_scale.y * (c2 * c3),
            inv_scale.y * (c1 * c3 * s2 + s1 * s3),
            0.0, 

            inv_scale.z * (c2 * s1),
            inv_scale.z * (-s2),
            inv_scale.z * (c1 * c2),
            0.0, 

            0.0, 0.0, 0.0, 0.0,
        ])
    }
}

static GAME_OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct GameObject {
    pub id: u32,
    pub transform: Transform3D,
    pub color: [f32; 3],
    pub model: Option<Arc<Model>>,
}

impl GameObject {
    pub fn new(model: Option<Arc<Model>>) -> GameObject {
        let id = GAME_OBJECT_COUNT.load(Ordering::SeqCst);
        GAME_OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);

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
    pub index_buffer: Option<Arc<DeviceLocalBuffer<[u32]>>>,
}

impl Model {
    pub fn new(renderer: &Renderer, vertices: Vec<Vertex>, indices: Option<Vec<u32>>) -> Self {

        let vertex_buffer = Model::create_device_local_buffer(renderer, vertices, BufferUsage::vertex_buffer());

        let index_buffer = if indices.is_some() { Some( 
            Model::create_device_local_buffer(renderer, indices.unwrap(), BufferUsage::index_buffer())
        )} else {
            None
        };

        Self {
            vertex_buffer,
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

    pub fn load_obj(renderer: &Renderer, filepath: &str) -> Model {
        let cornell_box = load_obj(
            filepath, 
            &tobj::GPU_LOAD_OPTIONS,
        );
        let (models, _materials) = cornell_box.expect("Failed to load obj");
        // let _materials = materials.expect("Failed to load materials");

        let mesh = &models[0].mesh;

        let has_normals = !mesh.normals.is_empty();
        let has_uvs = !mesh.texcoords.is_empty();
        let has_colors = !mesh.vertex_color.is_empty();

        let mut unique_vertices: HashMap<Vertex, u32> = HashMap::new();
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for i in &mesh.indices {
            let index = *i as usize;

            let position = [
                mesh.positions[index * 3 + 0],
                mesh.positions[index * 3 + 1],
                mesh.positions[index * 3 + 2],
            ];

            let mut normal = [0.0, 0.0, 0.0];
            if has_normals {
                normal = [
                    mesh.normals[index * 3 + 0],
                    mesh.normals[index * 3 + 1],
                    mesh.normals[index * 3 + 2],
                ];
            }

            let mut uv = [0.0, 0.0];
            if has_uvs {
                uv = [
                    mesh.texcoords[index * 2 + 0],
                    mesh.texcoords[index * 2 + 1],
                ];
            }

            let mut color = [1.0, 1.0, 1.0];
            if has_colors {
                color = [
                    mesh.vertex_color[index * 3 + 0],
                    mesh.vertex_color[index * 3 + 1],
                    mesh.vertex_color[index * 3 + 2],
                ];
            }

            let vertex = Vertex {
                position,
                normal,
                uv,
                color,
            };

            let index = unique_vertices.entry(vertex).or_insert_with(|| {
                vertices.push(vertex);
                vertices.len() as u32 - 1
            });
            indices.push(*index);
        }

        println!("{} unique vertices", vertices.len());

        Model::new(
            renderer, 
            vertices, 
            Some(indices), 
        )
    }
}