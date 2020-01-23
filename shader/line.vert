#version 460

layout (location = 0) in vec3 in_position;
layout (location = 1) in vec4 in_color;

layout (location = 0) out vec4 out_color;

layout (push_constant) uniform View {
    mat4 proj;
    mat4 view;
} push_constants;

void main() {
    out_color = in_color;
    gl_Position = push_constants.proj * push_constants.view * vec4(in_position, 1.0);
}