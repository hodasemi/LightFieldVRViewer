#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0) uniform accelerationStructureNV tlas;

layout(set = 2, binding = 0) readonly buffer PlaneVertices {
    vec3 position;
    float first_index;

    vec3 normal;
    float last_index;
} plane_vertices[ ];

layout(set = 2, binding = 1) readonly buffer PlaneImageInfos {
    float left;
    float right;
    float top;
    float bottom;

    vec2 center;

    uint image_index;

    uint padding[1];
} plane_image_infos[ ];

struct RayPayload {
	vec4 color;
	float distance;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

void main() {
    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);

    pay_load.color = vec4(0.8, 0.8, 0.5, 1.0);
    pay_load.distance = gl_HitTNV;
}