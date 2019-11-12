#version 460

layout (location = 0) in vec3 in_position;
layout (location = 1) in vec3 in_color;

layout (location = 0) out vec3 out_color;

layout (set = 0, binding = 0) uniform View {
    mat4 proj;
    mat4 view;
} view;

void main() {
    out_color = in_color;
    gl_Position = view.proj * view.view * vec4(in_position, 1.0);
}