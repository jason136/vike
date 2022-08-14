#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 color;
layout(location = 2) in vec3 normal;
layout(location = 3) in vec2 uv;

layout(location = 0) out vec3 fragColor;

layout(push_constant) uniform PushConstantData {
    mat4 transform;
    mat4 modelMatrix;
} push;

const vec3 lightDirection = normalize(vec3(1.0, -3.0, -1.0));

void main() {
    gl_Position = push.transform * vec4(position, 1.0);
    fragColor = color;
}