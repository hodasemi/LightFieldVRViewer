#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

const float INFINITY = 1.0 / 0.0;

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

struct InfoSelector {
    ivec4 indices;
    vec4 weights;
};

layout(set = 0, binding = 1) readonly buffer Planes {
    PlaneVertex vertices[ ];
} planes;

layout(set = 0, binding = 2) readonly buffer PlaneInfos {
    PlaneImageInfo image_infos[ ];
} plane_infos;

layout(set = 0, binding = 3) readonly buffer InfoSelectors {
    InfoSelector selectors[ ];
} info_selectors;

layout(set = 0, binding = 4) uniform sampler2D images[ ];

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

layout(location = 0) rayPayloadInNV RayPayload pay_load;
layout(location = 1) rayPayloadNV vec4 global_origin;
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

InfoSelector get_selector() {
    int index = gl_PrimitiveID;

    // there are 2 primitives per plane

    if ((index % 2) != 0) {
        index = index - 1;
    }

    index = index / 2;

    return info_selectors.selectors[index];
}

// calculate barycentrics of point in reference to the plane
vec2 calculate_barycentrics(Plane plane, vec3 point) {
    vec2 barycentrics;

    vec3 horizontal_direction = plane.top_right - plane.top_left;
    vec3 vertical_direction = plane.bottom_left - plane.top_left;

    barycentrics.x = distance_to_line(plane.top_left, vertical_direction, point)
        / length(horizontal_direction);

    barycentrics.y = distance_to_line(plane.top_left, horizontal_direction, point)
        / length(vertical_direction);

    return barycentrics;
}

bool check_inside(PlaneImageInfo image_info, vec2 bary) {
    return (bary.x >= image_info.left) &&
        (bary.x <= image_info.right) &&
        (bary.y >= image_info.top) &&
        (bary.y <= image_info.bottom);
}

vec2 normalized_uv(PlaneImageInfo image_info, vec2 bary) {
    float u = (bary.x - image_info.left) / (image_info.right - image_info.left);
    float v = (bary.y - image_info.top) / (image_info.bottom - image_info.top);

    // swap u and v
    return vec2(v, u);
}

vec4 single_image(PlaneImageInfo image_info, vec2 hit_bary) {
    vec2 uv = normalized_uv(image_info, hit_bary);

    return texture(images[nonuniformEXT(image_info.image_index)], uv);
}

void set_pay_load(vec4 color) {
    pay_load.color = color;
    pay_load.distance = gl_HitTNV;
}

void interpolate_images(Plane plane, InfoSelector info_selector, vec2 hit_bary) {
    // set distance as default to be missing
    pay_load.distance = -1.0;

    int number_of_images = 0;

    for (; number_of_images < 4; number_of_images++) {
        if (info_selector.indices[number_of_images] == -1) {
            break;
        }
    }

    if (number_of_images == 1) {
        PlaneImageInfo info = plane_infos.image_infos[info_selector.indices[0]];

        if (check_inside(info, hit_bary)) {
            set_pay_load(single_image(info, hit_bary));
        }
    } else if (number_of_images == 2) {
        vec4 color = vec4(0.0);

        for (int i = 0; i < 2; i++) {
            PlaneImageInfo info = plane_infos.image_infos[info_selector.indices[i]];

            if (check_inside(info, hit_bary)) {
                color += single_image(info, hit_bary) * info_selector.weights[i];
            }
        }

        set_pay_load(color);
    } else if (number_of_images == 4) {
        vec4 color = vec4(0.0);

        for (int i = 0; i < 4; i++) {
            PlaneImageInfo info = plane_infos.image_infos[info_selector.indices[i]];

            if (check_inside(info, hit_bary)) {
                color += single_image(info, hit_bary) * info_selector.weights[i];
            }
        }

        set_pay_load(color);
    }
}

void main() {
    Plane plane = get_plane();
    InfoSelector info_selector = get_selector();

    // TODO: check for backface

    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(plane, info_selector, calculate_barycentrics(plane, point));
}