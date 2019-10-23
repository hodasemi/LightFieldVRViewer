#version 460

layout (location = 0) in vec3 in_position;
layout (location = 1) in vec2 in_uv;

layout (location = 0) out vec2 out_uv;

layout (set = 1, binding = 0) uniform View {
    mat4 proj;
    mat4 view;
} view;

void main() {
    out_uv = in_uv;
    gl_Position = view.proj * view.view * vec4(in_position, 1.0);
}