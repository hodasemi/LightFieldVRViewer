#version 460
#extension GL_NV_ray_tracing : require

struct RayPayload {
	vec4 color;
	float distance;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;

void main() {
    pay_load.color = vec4(0.1, 0.1, 0.1, 1.0);
    pay_load.distance = -1.0;
}