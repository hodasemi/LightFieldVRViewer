#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

struct PlaneVertex {
    vec3 position;
    float first_index;

    vec3 normal;
    float last_index;
};

struct PlaneImageInfo {
    float left;
    float right;
    float top;
    float bottom;

    vec2 center;

    uint image_index;

    uint padding[1];
};

layout(set = 0, binding = 1) readonly buffer Planes {
    PlaneVertex vertices[ ];
} planes;

layout(set = 0, binding = 2) readonly buffer PlaneInfos {
    PlaneImageInfo image_infos[ ];
} plane_infos;

struct RayPayload {
	vec4 color;
	float distance;
};

struct Plane {
    vec3 top_left;
    vec3 top_right;
    vec3 bottom_left;
    vec3 bottom_right;

    vec3 normal;

    float first_index;
    float last_index;
};

struct PlaneBarycentrics {
    float x;
    float y;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

float distance_to_line(vec3 reference, vec3 normal, vec3 target) {
    return dot((target - reference), normal) / length(normal);
}

// Extracts all necessary information from gl_PrimitiveID
// and creates a Plane
Plane get_plane() {
    PlaneVertex v0, v1, v2, v5;

    // v3 and v4 are duplicates, therefore not required

    // check which triangle of the plane is hit
    if ((gl_PrimitiveID % 2) == 0) {
        v0 = planes.vertices[3 * gl_PrimitiveID];
        v1 = planes.vertices[3 * gl_PrimitiveID + 1];
        v2 = planes.vertices[3 * gl_PrimitiveID + 2];
        v5 = planes.vertices[3 * gl_PrimitiveID + 5];
    } else {
        v0 = planes.vertices[3 * gl_PrimitiveID - 3];
        v1 = planes.vertices[3 * gl_PrimitiveID - 2];
        v2 = planes.vertices[3 * gl_PrimitiveID - 1];
        v5 = planes.vertices[3 * gl_PrimitiveID + 2];
    }

    Plane plane;

    plane.top_left = v1.position;
    plane.top_right = v5.position;
    plane.bottom_left = v0.position;
    plane.bottom_right = v2.position;

    plane.normal = v0.normal;

    plane.first_index = v0.first_index;
    plane.last_index = v0.last_index;

    return plane;
}

PlaneBarycentrics calculate_barycentrics(Plane plane, vec3 point) {
    PlaneBarycentrics barycentrics;

    vec3 horizontal_direction = plane.top_right - plane.top_left;
    vec3 vertical_direction = plane.bottom_left - plane.top_left;

    barycentrics.x = distance_to_line(plane.top_left, vertical_direction, point);
    barycentrics.y = distance_to_line(plane.top_left, horizontal_direction, point);

    return barycentrics;
}

vec4 interpolate_images(Plane plane, PlaneBarycentrics barycentrics) {
    return vec4(1.0);
}

void main() {
    Plane plane = get_plane();

    // TODO: check for backface

    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    pay_load.color = interpolate_images(plane, calculate_barycentrics(plane, point));
    pay_load.distance = gl_HitTNV;
}