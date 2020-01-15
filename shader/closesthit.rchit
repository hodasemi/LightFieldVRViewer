#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

const float INFINITY = 1.0 / 0.0;

struct PlaneInfo {
    vec4 top_left;
    vec4 top_right;
    vec4 bottom_left;
    vec4 bottom_right;

    vec4 normal;

    ivec4 indices;
    vec4 weights;
};

layout(set = 0, binding = 1) readonly buffer PlaneInfos {
    PlaneInfo data[ ];
} plane_infos;

layout(set = 0, binding = 2) uniform sampler2D images[ ];

struct RayPayload {
	vec4 color;
	float distance;
    float cos;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
layout(location = 1) rayPayloadNV vec4 global_origin;
hitAttributeNV vec2 attribs;

float distance_to_line(vec3 reference, vec3 normal, vec3 target) {
    return dot((target - reference), normal) / length(normal);
}

PlaneInfo get_plane() {
    int index = gl_PrimitiveID;

    // there are 2 primitives per plane

    if ((index % 2) != 0) {
        index = index - 1;
    }

    index = index / 2;

    return plane_infos.data[index];
}

// calculate barycentrics of point in reference to the plane
vec2 calculate_barycentrics(PlaneInfo plane, vec3 point) {
    vec2 barycentrics;

    vec3 horizontal_direction = plane.top_right.xyz - plane.top_left.xyz;
    vec3 vertical_direction = plane.bottom_left.xyz - plane.top_left.xyz;

    barycentrics.x = distance_to_line(plane.top_left.xyz, vertical_direction, point)
        / length(horizontal_direction);

    barycentrics.y = distance_to_line(plane.top_left.xyz, horizontal_direction, point)
        / length(vertical_direction);

    return barycentrics;
}

// bool check_inside(PlaneImageInfo image_info, vec2 bary) {
//     return (bary.x >= image_info.left) &&
//         (bary.x <= image_info.right) &&
//         (bary.y >= image_info.top) &&
//         (bary.y <= image_info.bottom);
// }

// vec2 normalized_uv(PlaneImageInfo image_info, vec2 bary) {
//     float u = (bary.x - image_info.left) / (image_info.right - image_info.left);
//     float v = (bary.y - image_info.top) / (image_info.bottom - image_info.top);

//     // swap u and v
//     return vec2(v, u);
// }

// vec4 single_image(PlaneImageInfo image_info, vec2 hit_bary) {
//     // vec2 uv = normalized_uv(image_info, hit_bary);

//     vec2 uv = hit_bary.yx;

//     return texture(images[nonuniformEXT(image_info.image_index)], uv);
// }

vec4 single_image(int index, vec2 hit_bary) {
    // vec2 uv = normalized_uv(image_info, hit_bary);

    vec2 uv = hit_bary.yx;

    return texture(images[nonuniformEXT(index)], uv);
}

void set_pay_load(vec4 color) {
    pay_load.color = color;
    pay_load.distance = gl_HitTNV;
}

void interpolate_images(PlaneInfo plane, vec2 hit_bary) {
    if (plane.indices[0] == -1) {
        return;
    }

    vec4 color = vec4(0.0);

    for (int i = 0; i < 4; i++) {
        if (plane.indices[i] == -1) {
            break;
        }

        // PlaneImageInfo info = plane_infos.image_infos[plane.indices[i]];

        // if (check_inside(info, hit_bary)) {
            color += single_image(plane.indices[i], hit_bary) * plane.weights[i];
        // }
    }

    set_pay_load(color);
}

void main() {
    PlaneInfo plane = get_plane();

    pay_load.distance = 0.0;
    pay_load.color = vec4(0.0);

    float angle = dot(-plane.normal.xyz, gl_WorldRayDirectionNV);
    pay_load.cos = angle;

    if (angle < 0.0) {
        return;
    }

    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(plane, calculate_barycentrics(plane, point));
}