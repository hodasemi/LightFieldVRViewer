#version 460
#extension GL_NV_ray_tracing : require
#extension GL_EXT_nonuniform_qualifier : require

const float INFINITY = 1.0 / 0.0;

struct ImageBounds {
    float left;
    float right;
    float top;
    float bottom;
};

struct PlaneInfo {
    vec4 top_left;
    vec4 top_right;
    vec4 bottom_left;
    vec4 bottom_right;

    vec4 normal;

    ivec4 indices;
    vec4 bary;
    ImageBounds bounds[4];
};

layout(set = 0, binding = 1) readonly buffer PlaneInfos {
    PlaneInfo data[ ];
} plane_infos;

layout(set = 0, binding = 2) uniform sampler2D images[ ];

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

vec2 normalized_uv(ImageBounds bounds, vec2 bary) {
    float u = (bary.x - bounds.left) / (bounds.right - bounds.left);
    float v = (bary.y - bounds.top) / (bounds.bottom - bounds.top);

    // swap u and v
    return vec2(v, u);
}

bool check_inside(ImageBounds bounds, vec2 bary) {
    return (bary.x >= bounds.left) &&
        (bary.x <= bounds.right) &&
        (bary.y >= bounds.top) &&
        (bary.y <= bounds.bottom);
}

vec4 single_image(int index, vec2 hit_bary, ImageBounds bounds) {
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

void interpolate_images(PlaneInfo plane, vec2 hit_bary) {
    vec4 color = vec4(0.0);

    if (plane.indices[0] == -1) {
        return;
    } else if (plane.indices[1] == -1) {
        color = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
    } else if (plane.indices[2] == -1) {
        vec4 first = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
        vec4 second = single_image(plane.indices[1], hit_bary, plane.bounds[1]);

        color = linear(plane.bary.x, first, second);
    } else {
        vec4 first = single_image(plane.indices[0], hit_bary, plane.bounds[0]);
        vec4 second = single_image(plane.indices[1], hit_bary, plane.bounds[1]);
        vec4 third = single_image(plane.indices[2], hit_bary, plane.bounds[2]);
        vec4 fourth = single_image(plane.indices[3], hit_bary, plane.bounds[3]);

        color = bilinear(plane.bary.xy, first, second, third, fourth);
    }

    set_pay_load(color);
}

void main() {
    PlaneInfo plane = get_plane();

    pay_load.distance = 0.0;
    pay_load.color = vec4(0.0);
    pay_load.factor = plane.bary.z;

    float angle = dot(-plane.normal.xyz, gl_WorldRayDirectionNV);
    pay_load.cos = angle;

    if (angle < 0.0) {
        return;
    }

    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(plane, calculate_barycentrics(plane, point));
}