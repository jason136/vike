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
    vec4 ambientLightColor;
    vec3 lightPosition;
    vec4 lightColor;
} ubo;

const float ambient = 0.02;

void main() {
    vec4 positionWorld = push.modelMatrix * vec4(position, 1.0);
    gl_Position = ubo.projectionView * positionWorld;

    vec3 normalWorldSpace = normalize(mat3(push.normalMatrix) * normal);

    vec3 lightDirection = ubo.lightPosition - positionWorld.xyz;
    float attenuation = 1.0 / dot(lightDirection, lightDirection);

    vec3 lightColor = ubo.lightColor.xyz * ubo.lightColor.w * attenuation;
    vec3 ambientLight = ubo.ambientLightColor.xyz * ubo.ambientLightColor.w;
    vec3 diffuseLight = lightColor * max(dot(normalWorldSpace, normalize(lightDirection)), 0);

    fragColor = (diffuseLight + ambientLight) * color;
}