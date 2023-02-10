#version 450

layout (location = 0) in vec2 fragOffset;
layout (location = 0) out vec4 outColor;

struct PointLight {
    vec4 position;
    vec4 color;
};

layout(set = 0, binding = 0) uniform UniformBufferData {
    mat4 projection;
    mat4 view;
    mat4 inverseView;
    vec4 ambientLightColor;
    PointLight pointLights[10];
    int numLights;
} ubo;

layout(push_constant) uniform PushConstantData {
    vec4 position;
    vec4 color;
    float radius;
} push;

const float PI = 3.14159265359;

void main() {
    float dis = sqrt(dot(fragOffset, fragOffset));
    if (dis >= 1.0) {
        discard;
    }

    float cosDis = 0.5 * (cos(dis * PI) + 1.0);

    outColor = vec4(push.color.xyz + cosDis, cosDis);
    // outColor = vec4(push.color.xyz, cosDis);
}