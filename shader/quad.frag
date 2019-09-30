#version 460

layout (location = 0) in vec2 in_uv;

layout (location = 0) out vec4 out_frag_color;

layout (set = 0, binding = 0) uniform sampler2D image;

void main() {
    out_frag_color = texture(image, in_uv);
}