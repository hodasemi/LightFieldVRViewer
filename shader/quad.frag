#version 460
#extension GL_EXT_nonuniform_qualifier : require

layout (location = 0) in vec2 in_uv;
layout (location = 1) flat in uint in_image_index;

layout (location = 0) out vec4 out_frag_color;

layout (set = 1, binding = 0) uniform sampler2D images[ ];

void main() {
    out_frag_color = texture(images[nonuniformEXT(in_image_index)], in_uv);
}