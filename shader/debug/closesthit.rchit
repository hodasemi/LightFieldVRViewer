#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

const float INFINITY = 1.0 / 0.0;

layout(set = 0, binding = 1) readonly buffer Plane {
    vec4 top_left;
    vec4 top_right;
    vec4 bottom_left;
    vec4 bottom_right;

    vec4 normal;
} plane_info;

struct ImageInfo {
    vec4 bound;
    int image_index;
};

layout(set = 0, binding = 2) readonly buffer ImageInfos {
    ImageInfo data[ ];
} image_infos;

layout(set = 0, binding = 3) uniform sampler2D images[ ];

struct RayPayload {
	vec4 color;
	float distance;
    float cos;
    float factor;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

float distance_to_line(vec3 reference, vec3 normal, vec3 target) {
    return dot((target - reference), normal) / length(normal);
}

// calculate barycentrics of point in reference to the plane
vec2 calculate_barycentrics(vec3 point) {
    vec2 barycentrics;

    vec3 horizontal_direction = plane_info.top_right.xyz - plane_info.top_left.xyz;
    vec3 vertical_direction = plane_info.bottom_left.xyz - plane_info.top_left.xyz;

    barycentrics.x = distance_to_line(plane_info.top_left.xyz, vertical_direction, point)
        / length(horizontal_direction);

    barycentrics.y = distance_to_line(plane_info.top_left.xyz, horizontal_direction, point)
        / length(vertical_direction);

    return barycentrics;
}

vec2 normalized_uv(vec4 bounds, vec2 bary) {
    float u = (bary.x - bounds.x) / (bounds.y - bounds.x);
    float v = (bary.y - bounds.z) / (bounds.w - bounds.z);

    return vec2(v, u);
}

bool check_inside(vec4 bounds, vec2 bary) {
    return (bary.x >= bounds.x) &&
        (bary.x <= bounds.y) &&
        (bary.y >= bounds.z) &&
        (bary.y <= bounds.w);
}

vec4 single_image(int index, vec2 hit_bary, vec4 bounds) {
    if (check_inside(bounds, hit_bary)) {
        vec2 uv = normalized_uv(bounds, hit_bary);
        return texture(images[nonuniformEXT(index)], uv);
    } else {
        return vec4(0.0);
    }
}

void set_pay_load(vec4 color) {
    pay_load.color = color;
    pay_load.distance = gl_HitTNV;
}

vec4 linear(
    float factor,
    vec4 first_color,
    vec4 second_color
) {
    return factor * first_color + (1.0 - factor) * second_color;
}

vec4 bilinear(
    vec2 bary,
    vec4 top_left_color,
    vec4 top_right_color,
    vec4 bottom_left_color,
    vec4 bottom_right_color
) {
    vec4 left = linear(bary.y, top_left_color, bottom_left_color);
    vec4 right = linear(bary.y, top_right_color, bottom_right_color);

    return linear(bary.x, left, right);
}

void interpolate_images(vec2 hit_bary) {
    vec4 color = vec4(0.0);
    int i = 0;

    for (; i < image_infos.data.length(); i++) {
        color += single_image(
            image_infos.data[i].image_index,
            hit_bary,
            image_infos.data[i].bound
        );
    }

    set_pay_load(color / float(i));
}

void main() {
    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(calculate_barycentrics(point));
}