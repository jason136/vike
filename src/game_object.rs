use crate::texture::Texture;

use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, UnitQuaternion, Vector3};
use std::ops::Range;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub trait Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3];
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    // pub uv: [f32; 2],
}

impl Vertex for ModelVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

// impl PartialEq for Vertex {
//     fn eq(&self, other: &Self) -> bool {
//         self.position == other.position
//             && self.color == other.color
//             && self.normal == other.normal
//             && self.uv == other.uv
//     }
// }
// impl Eq for Vertex {}
// impl Hash for Vertex {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.as_bytes().hash(state);
//     }
// }

#[derive(Clone)]
pub struct Transform3D {
    pub translation: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub rotation: Vector3<f32>,
}

#[allow(dead_code)]
#[rustfmt::skip]
impl Transform3D {
    pub fn new() -> Self {
        Transform3D {
            translation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
        }
    }

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

#[derive(Clone, Copy)]
pub struct PointLight {
    pub light_intensity: f32,
}

static GAME_OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct GameObject {
    pub id: u32,
    pub transform: Transform3D,
    pub color: Vector3<f32>,
    pub model: Option<Arc<Model>>,
    pub point_light: Option<PointLight>,
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
            color: Vector3::new(0.0, 0.0, 0.0),
            model,
            point_light: None,
        }
    }

    pub fn new_point_light(light_intensity: f32, radius: f32, color: Vector3<f32>) -> GameObject {
        let id = GAME_OBJECT_COUNT.load(Ordering::SeqCst);
        GAME_OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);

        let transform = Transform3D {
            translation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(radius, 1.0, 1.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
        };

        GameObject {
            id,
            transform,
            color,
            model: None,
            point_light: Some(PointLight { light_intensity }),
        }
    }
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model(&mut self, model: &'a Model, camera_bind_group: &'a wgpu::BindGroup);
    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(&mut self, model: &'b Model, camera_bind_group: &'b wgpu::BindGroup) {
        self.draw_model_instanced(model, 0..1, camera_bind_group);
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group);
        }
    }
}

pub struct Instance {
    pub position: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (Matrix4::new_translation(&self.position) * Matrix4::from(self.rotation)).into(),
        }
    }
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}
