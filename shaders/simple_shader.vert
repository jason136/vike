#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 normal;
layout(location = 3) in vec2 uv;

layout(location = 0) out vec3 fragColor;

layout(push_constant) uniform PushConstantData {
    mat4 modelMatrix;
    mat4 normalMatrix;
} push;

layout(set = 0, binding = 0) uniform UniformBufferData {
    mat4 projectionView;
    vec3 lightDirection;
} ubo;

const float ambient = 0.02;

void main() {
    gl_Position = ubo.projectionView * push.modelMatrix * vec4(position, 1.0);

    vec3 normalWorldSpace = normalize(mat3(push.normalMatrix) * normal);

    float lightIntensity = ambient + max(dot(normalWorldSpace, ubo.lightDirection), 0);

    fragColor = lightIntensity * color;
}