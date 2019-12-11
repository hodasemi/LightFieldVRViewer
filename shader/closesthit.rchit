#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0) uniform accelerationStructureNV tlas;

struct RayPayload {
	vec4 color;
	float distance;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

void main() {
    const vec3 barycentrics = vec3(1.0f - attribs.x - attribs.y, attribs.x, attribs.y);

    pay_load.color = vec4(0.1, 0.1, 0.1, 1.0);
    pay_load.distance = -1.0;
}