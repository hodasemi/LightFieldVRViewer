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

    uint first_index;
    uint last_index;
};

struct PlaneBarycentrics {
    float x;
    float y;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

// simple
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

    plane.first_index = uint(v0.first_index);
    plane.last_index = uint(v0.last_index);

    return plane;
}

// Basic line - plane - intersection
vec3 calculate_orthogonal_point(Plane plane) {
    float numerator = dot((plane.top_left - gl_WorldRayOriginNV), plane.normal);
    float denominator = dot(-plane.normal, plane.normal);

    float distance = numerator / denominator;

    return gl_WorldRayOriginNV + (-plane.normal * distance);
}

// calculate barycentrics of point in reference to the plane
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

PlaneImageInfo find_closest_bottom_right(uint start_index, uint end_index, PlaneBarycentrics bary) {
    float mininal_distance = 2.0;
    PlaneImageInfo image_info;

    for (uint i = start_index; i < end_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - bary.x;

        // skip everything left of current x
        if (x_diff < 0.0) {
            continue;
        }

        float y_diff = info.center.y - bary.y;

        // skip everything above the current y
        if (y_diff < 0.0) {
            continue;
        }

        float new_distance = x_diff + y_diff;

        if (new_distance < mininal_distance) {
            mininal_distance = new_distance;
            image_info = info;
        }
    }

    return image_info;
}

PlaneImageInfo find_closest_top_right(uint start_index, uint end_index, PlaneBarycentrics bary) {
    float mininal_distance = 2.0;
    PlaneImageInfo image_info;

    for (uint i = start_index; i < end_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - bary.x;

        // skip everything left of current x
        if (x_diff < 0.0) {
            continue;
        }

        float y_diff = info.center.y - bary.y;

        // skip everything below the current y
        if (y_diff > 0.0) {
            continue;
        }

        float new_distance = x_diff - y_diff;

        if (new_distance < mininal_distance) {
            mininal_distance = new_distance;
            image_info = info;
        }
    }

    return image_info;
}

PlaneImageInfo find_closest_bottom_left(uint start_index, uint end_index, PlaneBarycentrics bary) {
    float mininal_distance = 2.0;
    PlaneImageInfo image_info;

    for (uint i = start_index; i < end_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - bary.x;

        // skip everything right of current x
        if (x_diff > 0.0) {
            continue;
        }

        float y_diff = info.center.y - bary.y;

        // skip everything above the current y
        if (y_diff < 0.0) {
            continue;
        }

        float new_distance = y_diff - x_diff;

        if (new_distance < mininal_distance) {
            mininal_distance = new_distance;
            image_info = info;
        }
    }

    return image_info;
}

PlaneImageInfo find_closest_top_left(uint start_index, uint end_index, PlaneBarycentrics bary) {
    float mininal_distance = 2.0;
    PlaneImageInfo image_info;

    for (uint i = start_index; i < end_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - bary.x;

        // skip everything right of current x
        if (x_diff > 0.0) {
            continue;
        }

        float y_diff = info.center.y - bary.y;

        // skip everything below the current y
        if (y_diff > 0.0) {
            continue;
        }


        float new_distance = -(x_diff + y_diff);

        if (new_distance < mininal_distance) {
            mininal_distance = new_distance;
            image_info = info;
        }
    }

    return image_info;
}

vec4 interpolate_images(Plane plane, PlaneBarycentrics hit_bary, PlaneBarycentrics pov_bary) {
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
            PlaneImageInfo image_info = find_closest_top_left(
                plane.first_index,
                plane.last_index,
                pov_bary
            );

            float u = (hit_bary.x - image_info.left) / (image_info.right - image_info.left);
            float v = (hit_bary.y - image_info.top) / (image_info.bottom - image_info.top);

            return texture(images[nonuniformEXT(image_info.image_index)], vec2(u, v));
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
    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    pay_load.color = interpolate_images(
        plane,
        calculate_barycentrics(plane, point),
        calculate_barycentrics(plane, viewer_point)
    );

    pay_load.distance = gl_HitTNV;
}