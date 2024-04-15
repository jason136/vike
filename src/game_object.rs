use crate::renderer::Renderer;
use crate::resources::load_model;
use crate::texture::Texture;
use crate::MAX_LIGHTS;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use std::collections::btree_map::{Iter, IterMut};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::Arc;

pub struct GameObjectStore {
    objects: BTreeMap<String, GameObject>,
    models: HashMap<String, Arc<Model>>,
    lights: BTreeMap<String, GameLight>,
    models_to_objects: BTreeMap<String, Vec<String>>,
    models_to_lights: BTreeMap<String, Vec<String>>,
}

pub struct GameObjectInstances {
    pub objects: Vec<(Arc<Model>, Range<u32>)>,
    pub lights: Vec<(Arc<Model>, Range<u32>)>,
    pub instances: Vec<InstanceRaw>,
}

impl GameObjectStore {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            models: HashMap::new(),
            lights: BTreeMap::new(),
            models_to_objects: BTreeMap::new(),
            models_to_lights: BTreeMap::new(),
        }
    }

    pub async fn load_model(&mut self, filename: &str, renderer: &Renderer) -> Result<Arc<Model>> {
        if let Some(model) = self.models.get(filename) {
            Ok(model.clone())
        } else {
            let model = Arc::new(load_model(filename, renderer).await?);
            self.models.insert(filename.to_string(), model.clone());
            Ok(model)
        }
    }

    pub fn new_game_object(
        &mut self,
        name: &str,
        transform: Transform3D,
        model: Option<Arc<Model>>,
    ) {
        let obj = GameObject {
            transform,
            model: model.clone(),
        };

        self.objects.insert(name.to_string(), obj);
        if let Some(model) = model {
            if let Some(vec) = self.models_to_objects.get_mut(&model.name) {
                vec.push(name.to_string());
            } else {
                self.models_to_objects
                    .insert(model.name.clone(), vec![name.to_string()]);
            }
        }
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
            model: model.clone(),
            color,
            intensity,
        };

        self.lights.insert(name.to_string(), light);
        if let Some(model) = model {
            if let Some(vec) = self.models_to_lights.get_mut(&model.name) {
                vec.push(name.to_string());
            } else {
                self.models_to_lights
                    .insert(model.name.clone(), vec![name.to_string()]);
            }
        }
    }

    pub fn delete_object(&mut self, name: &str) -> Option<GameObject> {
        let object = self.objects.remove(name)?;
        if let Some(model) = &object.model {
            let vec = self.models_to_objects.get_mut(&model.name).unwrap();
            let index = vec.iter().position(|x| *x == model.name).unwrap();
            vec.swap_remove(index);
            if vec.is_empty() {
                self.models_to_objects.remove(&model.name);
            }
        }

        Some(object)
    }

    pub fn delete_light(&mut self, name: &str) -> Option<GameLight> {
        let light = self.lights.remove(name)?;
        if let Some(model) = &light.model {
            let vec = self.models_to_lights.get_mut(&model.name).unwrap();
            let index = vec.iter().position(|x| *x == model.name).unwrap();
            vec.swap_remove(index);
            if vec.is_empty() {
                self.models_to_lights.remove(&model.name);
            }
        }

        Some(light)
    }

    pub fn light_uniform(&self) -> LightUniform {
        let mut light_uniform = LightUniform::new();
        for (index, light) in self.lights.values().enumerate() {
            if index >= MAX_LIGHTS {
                break;
            }
            light_uniform.lights[index] = Light {
                position: light.transform.position.into(),
                color: light.color.into(),
                intensity: light.intensity,
                _padding: 0,
            };
        }

        light_uniform.num_lights = std::cmp::max(self.lights.len(), MAX_LIGHTS) as u32;
        light_uniform
    }

    pub fn instances(&self) -> GameObjectInstances {
        let mut instances = Vec::new();
        let mut object_models = Vec::new();
        let mut light_models = Vec::new();

        let (mut start, mut end) = (0, 0);
        let mut curr_model = Arc::new(Model {
            name: "".to_string(),
            meshes: Vec::new(),
            materials: Vec::new(),
        });
        for (model_name, object_names) in &self.models_to_objects {
            for object_name in object_names {
                let object = self.objects.get(object_name).unwrap();
                instances.push(object.transform.to_raw_instance());
                if &curr_model.name == model_name {
                    end += 1;
                } else {
                    if start != end {
                        object_models.push((curr_model, start..end));
                    }
                    curr_model = self.models.get(model_name).unwrap().clone();
                    (start, end) = (end, end + 1);
                }
            }
        }
        object_models.push((curr_model.clone(), start..end));
        start = end;
        for (model_name, light_names) in &self.models_to_lights {
            for light_name in light_names {
                let light = self.lights.get(light_name).unwrap();
                let mut instance = light.transform.to_raw_instance();
                instance.normal[0] = light.color.into();
                instances.push(instance);
                if &curr_model.name == model_name {
                    end += 1;
                } else {
                    if start != end {
                        light_models.push((curr_model, start..end));
                    }
                    curr_model = self.models.get(model_name).unwrap().clone();
                    (start, end) = (end, end + 1);
                }
            }
        }
        light_models.push((curr_model.clone(), start..end));

        GameObjectInstances {
            objects: object_models,
            lights: light_models,
            instances,
        }
    }

    pub fn object(&mut self, name: &str) -> Option<&mut GameObject> {
        self.objects.get_mut(name)
    }

    pub fn light(&mut self, name: &str) -> Option<&mut GameLight> {
        self.lights.get_mut(name)
    }

    pub fn objects(&self) -> Iter<'_, String, GameObject> {
        self.objects.iter()
    }

    pub fn lights(&self) -> Iter<'_, String, GameLight> {
        self.lights.iter()
    }

    pub fn objects_mut(&mut self) -> IterMut<'_, String, GameObject> {
        self.objects.iter_mut()
    }

    pub fn lights_mut(&mut self) -> IterMut<'_, String, GameLight> {
        self.lights.iter_mut()
    }
}

pub struct GameObject {
    pub transform: Transform3D,
    pub model: Option<Arc<Model>>,
}

pub struct GameLight {
    pub transform: Transform3D,
    pub model: Option<Arc<Model>>,
    pub color: Vec3,
    pub intensity: f32,
}

#[derive(Clone, Debug)]
pub struct Transform3D {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

#[repr(C)]
#[derive(Copy, Clone, Zeroable, Pod)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

#[rustfmt::skip]
impl Transform3D {
    pub fn model(&self) -> Mat4 {
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

    pub fn normal(&self) -> Mat3 {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();
        let inv_scale = Vec3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);

        Mat3::from_cols_array(&[
            inv_scale.x * (c1 * c3 + s1 * s2 * s3),
            inv_scale.x * (c2 * s3),
            inv_scale.x * (c1 * s2 * s3 - c3 * s1),

            inv_scale.y * (c3 * s1 * s2 - c1 * s3),
            inv_scale.y * (c2 * c3),
            inv_scale.y * (c1 * c3 * s2 + s1 * s3),

            inv_scale.z * (c2 * s1),
            inv_scale.z * (-s2),
            inv_scale.z * (c1 * c2),
        ])
    }

    pub fn to_raw_instance(&self) -> InstanceRaw {
        // let rotation = Quat::from_euler(
        //     glam::EulerRot::XYZ,
        //     self.rotation.x,
        //     self.rotation.y,
        //     self.rotation.z,
        // );
        // let model = Mat4::from_scale(self.scale)
        //     * Mat4::from_translation(self.position)
        //     * Mat4::from_quat(rotation);
        // InstanceRaw {
        //     model: model.to_cols_array_2d(),
        //     normal: Mat3::from_quat(rotation).to_cols_array_2d(),
        // }
        InstanceRaw {
            model: self.model().to_cols_array_2d(),
            normal: self.normal().to_cols_array_2d(),
        }
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Transform3D {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
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

// #[allow(dead_code)]
pub struct Material {
    pub name: String,
    diffuse_texture: Texture,
    normal_texture: Texture,
    bind_group: wgpu::BindGroup,
}

#[allow(dead_code)]
pub struct Mesh {
    pub name: String,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_elements: u32,
    pub material: usize,
}

pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
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
    pub fn new(name: &str, meshes: Vec<Mesh>, materials: Vec<Material>) -> Self {
        Self {
            name: name.to_string(),
            meshes,
            materials,
        }
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

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
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
