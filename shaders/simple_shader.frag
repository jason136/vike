#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 1) in vec3 fragPosWorld;
layout(location = 2) in vec3 fragNormalWorld;

layout(location = 0) out vec4 outColor;

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

void main() {

  vec3 lightDirection = ubo.lightPosition - fragPosWorld;
  float attenuation = 1.0 / dot(lightDirection, lightDirection);

  vec3 lightColor = ubo.lightColor.xyz * ubo.lightColor.w * attenuation;
  vec3 ambientLight = ubo.ambientLightColor.xyz * ubo.ambientLightColor.w;
  vec3 diffuseLight = lightColor * max(dot(normalize(fragNormalWorld), normalize(lightDirection)), 0);

  outColor = vec4((diffuseLight + ambientLight) * fragColor, 1.0);
}