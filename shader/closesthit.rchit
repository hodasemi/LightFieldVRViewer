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

layout(set = 0, binding = 3) uniform sampler2D images[ ];

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

vec3 calculate_orthogonal_point(Plane plane) {
    // Basic line - plane - intersection
    float numerator = dot((plane.top_left - gl_WorldRayOriginNV), plane.normal);
    float denominator = dot(-plane.normal, plane.normal);

    float distance = numerator / denominator;

    return gl_WorldRayOriginNV + (-plane.normal * distance);
}

PlaneBarycentrics calculate_barycentrics(Plane plane, vec3 point) {
    PlaneBarycentrics barycentrics;

    vec3 horizontal_direction = plane.top_right - plane.top_left;
    vec3 vertical_direction = plane.bottom_left - plane.top_left;

    barycentrics.x = distance_to_line(plane.top_left, vertical_direction, point)
        / length(horizontal_direction);

    barycentrics.y = distance_to_line(plane.top_left, horizontal_direction, point)
        / length(vertical_direction);

    return barycentrics;
}

vec4 interpolate_images(Plane plane, PlaneBarycentrics pov_bary) {
    /*
                            |                   |
        Above, Left Side    |       Above       |   Above, Right Side
                            |                   |
    ------------------------X-------------------------------------------
                            |///////////////////|
            Left Side       |////// Plane //////|       Right Side
                            |///////////////////|
    --------------------------------------------------------------------
                            |                   |
        Below, Left Side    |       Below       |   Below, Right Side
                            |                   |

    X - is our reference point
    */

    // check for Above
    if (pov_bary.y < 0.0) {
        // check horizontal axis
        if (pov_bary.x < 0.0) {
            // Above, Left Side

            return vec4(1.0, 1.0, 0.0, 1.0);
        } else if (pov_bary.x > 1.0) {
            // Above, Right Side

            return vec4(1.0, 0.0, 1.0, 1.0);
        } else {
            // Above Center

            return vec4(1.0, 0.0, 0.0, 1.0);
        }
    }
    // check for below
    else if (pov_bary.y > 1.0) {
        // check horizontal axis
        if (pov_bary.x < 0.0) {
            // Below, Left Side

            return vec4(0.0, 1.0, 1.0, 1.0);
        } else if (pov_bary.x > 1.0) {
            // Below, Right Side

            return vec4(0.5, 1.0, 0.5, 1.0);
        } else {
            // Below Center

            return vec4(0.0, 1.0, 0.0, 1.0);
        }
    }
    // we are in the center, vertically
    else {
        // check horizontal axis
        if (pov_bary.x < 0.0) {
            // Left Side

            return vec4(0.0, 0.5, 1.0, 1.0);
        } else if (pov_bary.x > 1.0) {
            // Right Side

            return vec4(0.0, 0.5, 1.0, 1.0);
        } else {
            // We hit the plane
        }
    }

    // dummy
    return vec4(1.0);
}

void main() {
    Plane plane = get_plane();

    // TODO: check for backface

    vec3 viewer_point = calculate_orthogonal_point(plane);

    pay_load.color = interpolate_images(
        plane,
        calculate_barycentrics(plane, viewer_point)
    );

    pay_load.distance = gl_HitTNV;
}