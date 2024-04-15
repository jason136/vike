use crate::texture::Texture;
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

pub struct GameObjectStore {
    pub objects: HashMap<String, GameObject>,
    pub lights: Vec<(String, GameLight)>,
}

impl GameObjectStore {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            lights: Vec::new(),
        }
    }

    pub fn new_game_object(
        &mut self,
        name: &str,
        transform: Transform3D,
        model: Option<Arc<Model>>,
    ) {
        let obj = GameObject { transform, model };

        self.objects.insert(name.to_string(), obj);
    }

    pub fn new_light(
        &mut self,
        name: &str,
        transform: Transform3D,
        model: Option<Arc<Model>>,
        color: Vec3,
        intensity: f32,
    ) {
        let light = GameLight {
            transform,
            model,
            color,
            intensity,
        };

        self.lights.push((name.to_string(), light));
    }

    pub fn get_object(&mut self, name: &str) -> Option<&mut GameObject> {
        self.objects.get_mut(name)
    }

    pub fn get_light(&mut self, name: &str) -> Option<&mut GameLight> {
        self.lights
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, l)| l)
    }

    pub fn get_light_uniform(&self) -> LightUniform {
        let mut light_uniform = LightUniform::new();

        for i in 0..MAX_LIGHTS {
            if let Some(light) = self.lights.get(i).map(|(_, l)| l) {
                light_uniform.lights[i] = Light {
                    position: light.transform.position.into(),
                    color: light.color.into(),
                    intensity: light.intensity,
                    _padding: 0,
                };
            } else {
                break;
            }
        }

        light_uniform.num_lights = std::cmp::max(self.lights.len(), MAX_LIGHTS) as u32;
        light_uniform
    }
}

pub struct GameObject {
    pub transform: Transform3D,
    pub model: Option<Arc<Model>>,
}

const MAX_LIGHTS: usize = 128;

pub struct GameLight {
    pub transform: Transform3D,
    pub model: Option<Arc<Model>>,
    pub color: Vec3,
    pub intensity: f32,
}

#[derive(Clone, Debug)]
pub struct Transform3D {
    pub position: Vec3,
    pub scale: Vec3,
    pub rotation: Vec3,
}

#[allow(dead_code)]
#[rustfmt::skip]
impl Transform3D {
    pub fn mat4(&self) -> Mat4 {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();

        Mat4::from_cols_array(&[
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

            self.position.x,
            self.position.y,
            self.position.z,
            1.0,
        ])
    }

    pub fn normal_matrix(&self) -> Mat4 {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();
        let inv_scale = Vec3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);

        Mat4::from_cols_array(&[
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

impl Default for Transform3D {
    fn default() -> Self {
        Transform3D {
            position: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            rotation: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Zeroable, Pod)]
pub struct Light {
    pub position: [f32; 3],
    _padding: u32,
    pub color: [f32; 3],
    pub intensity: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Zeroable, Pod)]
pub struct LightUniform {
    pub num_lights: u32,
    _padding: [u32; 3],
    pub lights: [Light; MAX_LIGHTS],
}

impl LightUniform {
    pub fn new() -> Self {
        Self {
            num_lights: 0,
            _padding: [0; 3],
            lights: [Light {
                position: [0.0; 3],
                _padding: 0,
                color: [0.0; 3],
                intensity: 0.0,
            }; MAX_LIGHTS],
        }
    }
}

#[derive(Debug)]
// #[allow(dead_code)]
pub struct Material {
    pub name: String,
    diffuse_texture: Texture,
    normal_texture: Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Mesh {
    pub name: String,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_elements: u32,
    material: usize,
}

#[derive(Debug)]
pub struct Model {
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
}

impl Mesh {
    pub fn new(
        name: &str,
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        num_elements: u32,
        material: usize,
    ) -> Self {
        Self {
            name: String::from(name),
            vertex_buffer,
            index_buffer,
            num_elements,
            material,
        }
    }
}

impl Material {
    pub fn new(
        device: &wgpu::Device,
        name: &str,
        diffuse_texture: Texture,
        normal_texture: Texture,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
            ],
            label: Some(name),
        });

        Self {
            name: String::from(name),
            diffuse_texture,
            normal_texture,
            bind_group,
        }
    }
}

impl Model {
    pub fn new(meshes: Vec<Mesh>, materials: Vec<Material>) -> Self {
        Self { meshes, materials }
    }
}

#[allow(dead_code)]
pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model_instanced_with_material(
        &mut self,
        model: &'a Model,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
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
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(
                mesh,
                material,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }

    fn draw_model_instanced_with_material(
        &mut self,
        model: &'b Model,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_mesh_instanced(
                mesh,
                material,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}

#[allow(dead_code)]
pub trait DrawLight<'a> {
    fn draw_light_mesh(
        &mut self,
        mesh: &'a Mesh,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        // self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        // self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        // self.set_bind_group(0, &material.bind_group, &[]);
        // self.set_bind_group(1, camera_bind_group, &[]);
        // self.set_bind_group(2, light_bind_group, &[]);
        // self.draw_indexed(0..mesh.num_elements, 0, instances);
        
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_light_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }
    fn draw_light_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(
                mesh,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}

pub struct Instance {
    pub position: Vec3,
    pub rotation: Quat,
}

#[repr(C)]
#[derive(Copy, Clone, Zeroable, Pod)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        let model = Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation);
        InstanceRaw {
            model: model.to_cols_array_2d(),
            normal: Mat3::from_quat(self.rotation).to_cols_array_2d(),
        }
    }
}

impl Vertex for InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
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
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
