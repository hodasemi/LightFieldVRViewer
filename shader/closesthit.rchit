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
    vec4 bary;
    vec4 bounds[4];
};

layout(set = 0, binding = 1) readonly buffer PlaneInfos {
    PlaneInfo data[ ];
} plane_infos;

layout(set = 0, binding = 2) uniform sampler2D images[ ];

struct RayPayload {
	vec4 color;
	float distance;
    float factor;
};

layout(location = 0) rayPayloadInNV RayPayload pay_load;
hitAttributeNV vec2 attribs;

float distance_to_line(vec3 reference, vec3 normal, vec3 target) {
    return dot((target - reference), normal) / length(normal);
}

PlaneInfo get_plane() {
    return plane_infos.data[gl_InstanceID];
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

vec4 bilinear(
    vec4 top_left_color,
    vec4 top_right_color,
    vec4 bottom_left_color,
    vec4 bottom_right_color,
    vec2 bary
) {
    vec4 left = mix(top_left_color, bottom_left_color, bary.y);
    vec4 right = mix(top_right_color, bottom_right_color, bary.y);

    return mix(left, right, bary.x);
}

void interpolate_images(PlaneInfo plane, vec2 hit_bary) {
    vec4 color = vec4(0.0);

    if (plane.indices[0] == -1) {
        return;
    } else if (plane.indices[1] == -1) {
        color = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
    } else if (plane.indices[2] == -1) {
        vec4 first = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
        vec4 second = single_image(plane.indices[1], hit_bary, plane.bounds[1]);

        color = mix(first, second, plane.bary.x);
    } else {
        vec4 first = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
        vec4 second = single_image(plane.indices[1], hit_bary, plane.bounds[1]);
        vec4 third = single_image(plane.indices[2], hit_bary, plane.bounds[2]);
        vec4 fourth = single_image(plane.indices[3], hit_bary, plane.bounds[3]);

        color = bilinear(first, second, third, fourth, plane.bary.xy);
    }

    set_pay_load(color);
}

void main() {
    PlaneInfo plane = get_plane();

    pay_load.distance = 0.0;
    pay_load.color = vec4(0.0);
    pay_load.factor = plane.bary.z;

    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(plane, calculate_barycentrics(plane, point));
}