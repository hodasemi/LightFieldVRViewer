#version 460
#extension GL_NV_ray_tracing : require

struct RayPayload {
	vec4 color;
	float distance;
    float factor;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;

void main() {}
