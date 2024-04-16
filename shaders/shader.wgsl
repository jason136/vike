struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> camera: Camera;

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
}
struct LightUniform {
    numLights: u32,
    lights: array<Light, 128>,
}
@group(2) @binding(0)
var<uniform> lights: LightUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) tangent_view_position: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
    @location(5) world_normal: vec3<f32>,
 }

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    let world_normal = normalize(normal_matrix * model.normal);
    let world_tangent = normalize(normal_matrix * model.tangent);
    let world_bitangent = normalize(normal_matrix * model.bitangent);
    let tangent_matrix = transpose(mat3x3<f32>(
        world_tangent,
        world_bitangent,
        world_normal,
    ));

    let world_position = model_matrix * vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_position;
    out.world_position = world_position.xyz;
    out.tex_coords = model.tex_coords;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.world_tangent = world_tangent;
    out.world_bitangent = world_bitangent;
    out.world_normal = world_normal;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.tex_coords);

    let tangent_matrix = transpose(mat3x3<f32>(
        in.world_tangent,
        in.world_bitangent,
        in.world_normal,
    ));

    var result: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    for (var i: u32 = 0; i < lights.numLights; i = i + 1) {
        let light = lights.lights[i];
        let light_distance = length(in.world_position - light.position);
        let light_intensity = clamp(light.intensity / (light_distance * light_distance), 0.01, 10.0);

        let ambient_strength = 0.01 / f32(lights.numLights);
        let ambient_color = light.color * ambient_strength;

        let tangent_normal = object_normal.xyz * 2.0 - 1.0;
        let tangent_light_position = tangent_matrix * lights.lights[i].position;
        let tangent_position = tangent_matrix * in.world_position;
        let light_dir = normalize(tangent_light_position - tangent_position);
        let view_dir = normalize(in.tangent_view_position - tangent_position);
        let half_dir = normalize(view_dir + light_dir);

        let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
        let diffuse_color = light.color * diffuse_strength * light_intensity;

        let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 16.0);
        let specular_color = light.color * specular_strength * light_intensity;

        result = result + (ambient_color + diffuse_color + specular_color) * object_color.xyz;
    }
    
    return vec4<f32>(result, object_color.a);
}
