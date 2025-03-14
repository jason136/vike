use anyhow::Result;
use cfg_if::cfg_if;
use futures_lite::io::{BufReader, Cursor};
use glam::{Vec2, Vec3};
use tobj::LoadError;
use wgpu::util::DeviceExt;

use crate::{
    game_object::{Material, Mesh, Model, ModelVertex},
    renderer::Renderer,
    texture::Texture,
};

pub async fn load_texture(
    filename: &str,
    is_normal_map: bool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Texture> {
    let data = load_binary(filename).await?;
    Texture::from_bytes(&data, filename, is_normal_map, device, queue)
}

pub async fn load_model(filename: &str, renderer: &Renderer) -> Result<Model> {
    let obj_text = load_string(filename).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::futures::load_obj_buf(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            match p.to_str() {
                Some(path) => {
                    let mat_text = load_string(path)
                        .await
                        .map_err(|_| LoadError::GenericFailure)?;
                    tobj::futures::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text))).await
                }
                None => Ok(Default::default()),
            }
        },
    )
    .await?;

    let mut max_mat_id = 0;
    let meshes = models
        .into_iter()
        .map(|m| {
            let mut vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                })
                .collect::<Vec<ModelVertex>>();

            let indices = &m.mesh.indices;
            let mut triangles_included = vec![0; vertices.len()];

            for c in indices.chunks(3) {
                let v0 = vertices[c[0] as usize];
                let v1 = vertices[c[1] as usize];
                let v2 = vertices[c[2] as usize];

                let pos0: Vec3 = v0.position.into();
                let pos1: Vec3 = v1.position.into();
                let pos2: Vec3 = v2.position.into();

                let uv0: Vec2 = v0.tex_coords.into();
                let uv1: Vec2 = v1.tex_coords.into();
                let uv2: Vec2 = v2.tex_coords.into();

                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                vertices[c[0] as usize].tangent =
                    (tangent + Vec3::from(vertices[c[0] as usize].tangent)).into();
                vertices[c[1] as usize].tangent =
                    (tangent + Vec3::from(vertices[c[1] as usize].tangent)).into();
                vertices[c[2] as usize].tangent =
                    (tangent + Vec3::from(vertices[c[2] as usize].tangent)).into();
                vertices[c[0] as usize].bitangent =
                    (bitangent + Vec3::from(vertices[c[0] as usize].bitangent)).into();
                vertices[c[1] as usize].bitangent =
                    (bitangent + Vec3::from(vertices[c[1] as usize].bitangent)).into();
                vertices[c[2] as usize].bitangent =
                    (bitangent + Vec3::from(vertices[c[2] as usize].bitangent)).into();

                triangles_included[c[0] as usize] += 1;
                triangles_included[c[1] as usize] += 1;
                triangles_included[c[2] as usize] += 1;
            }

            for (i, n) in triangles_included.into_iter().enumerate() {
                let denom = 1.0 / n as f32;
                let v = &mut vertices[i];
                v.tangent = (Vec3::from(v.tangent) * denom).into();
                v.bitangent = (Vec3::from(v.bitangent) * denom).into();
            }

            let vertex_buffer =
                renderer
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{:?} Vertex Buffer", filename)),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
            let index_buffer =
                renderer
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{:?} Index Buffer", filename)),
                        contents: bytemuck::cast_slice(&m.mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
            let material_id = std::cmp::max(max_mat_id, m.mesh.material_id.unwrap_or(0));
            max_mat_id = material_id;

            Mesh::new(
                filename,
                vertex_buffer,
                index_buffer,
                m.mesh.indices.len() as u32,
                material_id,
            )
        })
        .collect();

    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture = match m.diffuse_texture.as_ref() {
            Some(filename) => {
                load_texture(filename, false, renderer.device(), renderer.queue()).await
            }
            None => Texture::default(false, renderer.device(), renderer.queue()),
        }?;

        let normal_texture = match m.normal_texture.as_ref() {
            Some(filename) => {
                load_texture(filename, true, renderer.device(), renderer.queue()).await
            }
            None => Texture::default(true, renderer.device(), renderer.queue()),
        }?;

        materials.push(Material::new(
            renderer.device(),
            &m.name,
            diffuse_texture,
            normal_texture,
            renderer.texture_bind_group_layout(),
        ));
    }

    while materials.len() <= max_mat_id {
        materials.push(Material::new(
            renderer.device(),
            "default",
            Texture::default(false, renderer.device(), renderer.queue()).unwrap(),
            Texture::default(true, renderer.device(), renderer.queue()).unwrap(),
            renderer.texture_bind_group_layout(),
        ));
    }

    Ok(Model::new(filename, meshes, materials))
}

#[cfg(target_arch = "wasm32")]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
static BASE_URL: OnceLock<String> = OnceLock::new();

#[cfg(target_arch = "wasm32")]
pub fn set_base_url(url: &str) -> Result<(), &'static str> {
    match BASE_URL.set(url.to_string()) {
        Ok(_) => Ok(()),
        Err(_) => Err("Base URL has already been set"),
    }
}

#[cfg(target_arch = "wasm32")]
async fn js_fetch(filename: &str) -> Result<web_sys::Response> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::js_sys;

    let url = match BASE_URL.get() {
        Some(base) => format!("{}/{}", base, filename),
        None => format!("http://localhost:8080/models/{}", filename),
    };

    let global = js_sys::global();

    let fetch_fn = js_sys::Function::from(
        js_sys::Reflect::get(&global, &"fetch".into())
            .map_err(|_| anyhow::anyhow!("fetch not found in global scope"))?,
    );

    let resp_value = JsFuture::from(
        fetch_fn
            .call1(&global, &url.into())
            .map_err(|e| anyhow::anyhow!("JS fetch call error: {:?}", e))?
            .dyn_into::<js_sys::Promise>()
            .map_err(|_| anyhow::anyhow!("JS fetch did not return a Promise"))?,
    )
    .await
    .map_err(|e| anyhow::anyhow!("JS fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("JS fetch not a Response"))?;

    if !resp.ok() {
        return Err(anyhow::anyhow!("JS fetch HTTP error: {}", resp.status()));
    }

    Ok(resp)
}

pub async fn load_string(filename: &str) -> Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            use wasm_bindgen_futures::JsFuture;

            let resp = js_fetch(filename).await?;
            let text = JsFuture::from(resp.text()
                .map_err(|e| anyhow::anyhow!("JS load text error: {:?}", e))?)
                .await
                .map_err(|e| anyhow::anyhow!("JS await error: {:?}", e))?;

            let txt = text.as_string()
                .ok_or_else(|| anyhow::anyhow!("JS response not a string"))?;
        } else {
            let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap())
                .join("models")
                .join(filename);
            let txt = async_fs::read_to_string(path).await?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(filename: &str) -> Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            use web_sys::js_sys;
            use wasm_bindgen_futures::JsFuture;

            let resp = js_fetch(filename).await?;
            let array_buffer = JsFuture::from(resp.array_buffer()
                .map_err(|e| anyhow::anyhow!("JS load buffer error: {:?}", e))?)
                .await
                .map_err(|e| anyhow::anyhow!("JS await error: {:?}", e))?;

            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            let data = uint8_array.to_vec();
        } else {
            let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap())
                .join("models")
                .join(filename);
            let data = async_fs::read(path).await?;
        }
    }

    Ok(data)
}
