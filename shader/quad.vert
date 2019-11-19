#version 460

layout (location = 0) in vec3 in_position;
layout (location = 1) in uint in_image_index;
layout (location = 2) in vec2 in_uv;

layout (location = 0) out vec2 out_uv;
layout (location = 1) out uint out_image_index;

layout (set = 0, binding = 0) uniform View {
    mat4 proj;
    mat4 view;
} view;

void main() {
    out_uv = in_uv;
    out_image_index = in_image_index;
    gl_Position = view.proj * view.view * vec4(in_position, 1.0);
}