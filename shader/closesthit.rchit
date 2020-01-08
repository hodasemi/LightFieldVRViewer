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

// Basic line - plane - intersection
vec3 calculate_orthogonal_point(Plane plane, vec3 origin) {
    float numerator = dot((plane.top_left - origin), plane.normal);
    float denominator = dot(-plane.normal, plane.normal);

    float distance = numerator / denominator;

    return origin + (-plane.normal * distance);
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

bool find_closest_bottom_right(
    Plane plane,
    vec2 pov_bary,
    vec2 hit_bary,
    out PlaneImageInfo image_info,
    out float distance
) {
    distance = INFINITY;
    bool info_found = false;

    for (uint i = plane.first_index; i < plane.last_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - pov_bary.x;

        // skip everything left of current x
        if (x_diff < 0.0) {
            continue;
        }

        float y_diff = info.center.y - pov_bary.y;

        // skip everything above the current y
        if (y_diff < 0.0) {
            continue;
        }

        float new_distance = x_diff + y_diff;

        if (new_distance < distance && check_inside(info, hit_bary)) {
            distance = new_distance;
            image_info = info;
            info_found = true;
        }
    }

    return info_found;
}

bool find_closest_top_right(
    Plane plane,
    vec2 pov_bary,
    vec2 hit_bary,
    out PlaneImageInfo image_info,
    out float distance
) {
    distance = INFINITY;
    bool info_found = false;

    for (uint i = plane.first_index; i < plane.last_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - pov_bary.x;

        // skip everything left of current x
        if (x_diff < 0.0) {
            continue;
        }

        float y_diff = info.center.y - pov_bary.y;

        // skip everything below the current y
        if (y_diff > 0.0) {
            continue;
        }

        float new_distance = x_diff - y_diff;

        if (new_distance < distance && check_inside(info, hit_bary)) {
            distance = new_distance;
            image_info = info;
            info_found = true;
        }
    }

    return info_found;
}

bool find_closest_bottom_left(
    Plane plane,
    vec2 pov_bary,
    vec2 hit_bary,
    out PlaneImageInfo image_info,
    out float distance
) {
    distance= INFINITY;
    bool info_found = false;

    for (uint i = plane.first_index; i < plane.last_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - pov_bary.x;

        // skip everything right of current x
        if (x_diff > 0.0) {
            continue;
        }

        float y_diff = info.center.y - pov_bary.y;

        // skip everything above the current y
        if (y_diff < 0.0) {
            continue;
        }

        float new_distance = y_diff - x_diff;

        if (new_distance < distance && check_inside(info, hit_bary)) {
            distance = new_distance;
            image_info = info;
            info_found = true;
        }
    }

    return info_found;
}

bool find_closest_top_left(
    Plane plane,
    vec2 pov_bary,
    vec2 hit_bary,
    out PlaneImageInfo image_info,
    out float distance
) {
    distance = INFINITY;
    bool info_found = false;

    for (uint i = plane.first_index; i < plane.last_index; i++) {
        PlaneImageInfo info = plane_infos.image_infos[nonuniformEXT(i)];

        float x_diff = info.center.x - pov_bary.x;

        // skip everything right of current x
        if (x_diff > 0.0) {
            continue;
        }

        float y_diff = info.center.y - pov_bary.y;

        // skip everything below the current y
        if (y_diff > 0.0) {
            continue;
        }

        float new_distance = -(x_diff + y_diff);

        if (new_distance < distance && check_inside(info, hit_bary)) {
            distance = new_distance;
            image_info = info;
            info_found = true;
        }
    }

    return info_found;
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

vec4 two_images(PlaneImageInfo first_info, float first_distance, PlaneImageInfo second_info, float second_distance, vec2 hit_bary) {
    vec2 first_uv = normalized_uv(first_info, hit_bary);
    vec2 second_uv = normalized_uv(second_info, hit_bary);

    float total_distance = first_distance + second_distance;

    // second_distance = (total_distance - first_distance)
    float first_weight = second_distance / total_distance;

    // first_distance = (total_distance - second_distance)
    float second_weight = first_distance / total_distance;

    vec4 first_color = texture(images[nonuniformEXT(first_info.image_index)], first_uv);
    vec4 second_color = texture(images[nonuniformEXT(second_info.image_index)], second_uv);

    return first_color * first_weight + second_color * second_weight;
}

float weight(float weight, float total, int amount) {
    return weight / total;
}

vec4 four_images(
    PlaneImageInfo first_info, float first_distance,
    PlaneImageInfo second_info, float second_distance,
    PlaneImageInfo third_info, float third_distance,
    PlaneImageInfo fourth_info, float fourth_distance,
    vec2 hit_bary
) {
    vec2 first_uv = normalized_uv(first_info, hit_bary);
    vec2 second_uv = normalized_uv(second_info, hit_bary);
    vec2 third_uv = normalized_uv(third_info, hit_bary);
    vec2 fourth_uv = normalized_uv(fourth_info, hit_bary);

    vec4 first_color = texture(images[nonuniformEXT(first_info.image_index)], first_uv);
    vec4 second_color = texture(images[nonuniformEXT(second_info.image_index)], second_uv);
    vec4 third_color = texture(images[nonuniformEXT(third_info.image_index)], third_uv);
    vec4 fourth_color = texture(images[nonuniformEXT(fourth_info.image_index)], fourth_uv);

    float total_distance = first_distance + second_distance + third_distance + fourth_distance;

    float first_weight = weight(first_distance, total_distance, 4);
    float second_weight = weight(second_distance, total_distance, 4);
    float third_weight = weight(third_distance, total_distance, 4);
    float fourth_weight = weight(fourth_distance, total_distance, 4);

    return first_color * first_weight
        + second_color * second_weight
        + third_color * third_weight
        + fourth_color * fourth_weight;
}

void set_pay_load(vec4 color) {
    pay_load.color = color;
    pay_load.distance = gl_HitTNV;
}

void interpolate_images(Plane plane, vec2 hit_bary, vec2 pov_bary) {
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
            // --------------------- Above, Left Side ---------------------
            PlaneImageInfo image_info;
            float distance;

            if (find_closest_bottom_right(plane, pov_bary, hit_bary, image_info, distance)) {
                set_pay_load(single_image(image_info, hit_bary));
                return;
            }
        } else if (pov_bary.x > 1.0) {
            // --------------------- Above, Right Side ---------------------
            PlaneImageInfo image_info;
            float distance;

            if (find_closest_bottom_left(plane, pov_bary, hit_bary, image_info, distance)) {
                set_pay_load(single_image(image_info, hit_bary));
                return;
            }
        } else {
            // --------------------- Above Center ---------------------
            PlaneImageInfo left_image_info;
            PlaneImageInfo right_image_info;
            float left_distance;
            float right_distance;

            // get both image infos
            bool left_found = find_closest_bottom_left(plane, pov_bary, hit_bary, left_image_info, left_distance);
            bool right_found = find_closest_bottom_right(plane, pov_bary, hit_bary, right_image_info, right_distance);

            if (left_found && right_found) {
                set_pay_load(two_images(left_image_info, left_distance, right_image_info, right_distance, hit_bary));
                return;
            } else if (left_found && !right_found) {
                set_pay_load(single_image(left_image_info, hit_bary));
                return;
            } else if (!left_found && right_found) {
                set_pay_load(single_image(right_image_info, hit_bary));
                return;
            }
        }
    }
    // check for below
    else if (pov_bary.y > 1.0) {
        // check horizontal axis
        if (pov_bary.x < 0.0) {
            // --------------------- Below, Left Side ---------------------
            PlaneImageInfo image_info;
            float distance;

            if (find_closest_top_right(plane, pov_bary, hit_bary, image_info, distance)) {
                set_pay_load(single_image(image_info, hit_bary));
                return;
            }
        } else if (pov_bary.x > 1.0) {
            // --------------------- Below, Right Side ---------------------
            PlaneImageInfo image_info;
            float distance;

            if (find_closest_top_left(plane, pov_bary, hit_bary, image_info, distance)) {
                set_pay_load(single_image(image_info, hit_bary));
                return;
            }
        } else {
            // --------------------- Below Center ---------------------
            PlaneImageInfo left_image_info;
            PlaneImageInfo right_image_info;
            float left_distance;
            float right_distance;

            // get both image infos
            bool left_found = find_closest_top_left(plane, pov_bary, hit_bary, left_image_info, left_distance);
            bool right_found = find_closest_top_right(plane, pov_bary, hit_bary, right_image_info, right_distance);

            if (left_found && right_found) {
                set_pay_load(two_images(left_image_info, left_distance, right_image_info, right_distance, hit_bary));
                return;
            } else if (left_found && !right_found) {
                set_pay_load(single_image(left_image_info, hit_bary));
                return;
            } else if (!left_found && right_found) {
                set_pay_load(single_image(right_image_info, hit_bary));
                return;
            }
        }
    }
    // we are in the center, vertically
    else {
        // check horizontal axis
        if (pov_bary.x < 0.0) {
            // --------------------- Left Side ---------------------
            PlaneImageInfo upper_image_info;
            PlaneImageInfo lower_image_info;
            float upper_distance;
            float lower_distance;

            // get both image infos
            bool upper_found = find_closest_top_right(plane, pov_bary, hit_bary, upper_image_info, upper_distance);
            bool lower_found = find_closest_bottom_right(plane, pov_bary, hit_bary, lower_image_info, lower_distance);

            if (upper_found && lower_found) {
                set_pay_load(two_images(upper_image_info, upper_distance, lower_image_info, lower_distance, hit_bary));
                return;
            } else if (upper_found && !lower_found) {
                set_pay_load(single_image(upper_image_info, hit_bary));
                return;
            } else if (!upper_found && lower_found) {
                set_pay_load(single_image(lower_image_info, hit_bary));
                return;
            }
        } else if (pov_bary.x > 1.0) {
            // --------------------- Right Side ---------------------
            PlaneImageInfo upper_image_info;
            PlaneImageInfo lower_image_info;
            float upper_distance;
            float lower_distance;

            // get both image infos
            bool upper_found = find_closest_top_left(plane, pov_bary, hit_bary, upper_image_info, upper_distance);
            bool lower_found = find_closest_bottom_left(plane, pov_bary, hit_bary, lower_image_info, lower_distance);

            if (upper_found && lower_found) {
                set_pay_load(two_images(upper_image_info, upper_distance, lower_image_info, lower_distance, hit_bary));
                return;
            } else if (upper_found && !lower_found) {
                set_pay_load(single_image(upper_image_info, hit_bary));
                return;
            } else if (!upper_found && lower_found) {
                set_pay_load(single_image(lower_image_info, hit_bary));
                return;
            }
        } else {
            // --------------------- We hit the plane ---------------------
            PlaneImageInfo upper_left_image_info;
            PlaneImageInfo upper_right_image_info;
            PlaneImageInfo lower_left_image_info;
            PlaneImageInfo lower_right_image_info;

            float upper_left_distance;
            float upper_right_distance;
            float lower_left_distance;
            float lower_right_distance;

            bool lower_left_found = find_closest_bottom_left(plane, pov_bary, hit_bary, lower_left_image_info, lower_left_distance);
            bool lower_right_found = find_closest_bottom_left(plane, pov_bary, hit_bary, lower_right_image_info, lower_right_distance);
            bool upper_left_found = find_closest_bottom_left(plane, pov_bary, hit_bary, upper_left_image_info, upper_left_distance);
            bool upper_right_found = find_closest_bottom_left(plane, pov_bary, hit_bary, upper_right_image_info, upper_right_distance);

            // center - all required
            if (lower_left_found && lower_right_found && upper_right_found && upper_left_found) {
                set_pay_load(four_images(
                    lower_left_image_info, lower_left_distance,
                    lower_right_image_info, lower_right_distance,
                    upper_left_image_info, upper_left_distance,
                    upper_right_image_info, upper_right_distance,
                    hit_bary
                ));
                return;
            }


            // left top corner - only bottom right
            else if (!upper_left_found && !upper_right_found && !lower_left_found && lower_right_found) {
                set_pay_load(single_image(lower_right_image_info, hit_bary));
                return;
            }
            // left bottom corner - only top right
            else if (!upper_left_found && !lower_left_found && !lower_right_found && upper_right_found) {
                set_pay_load(single_image(upper_right_image_info, hit_bary));
                return;
            }
            // right top corner - only bottom left
            else if (!upper_right_found && !upper_left_found && !lower_right_found && lower_left_found) {
                set_pay_load(single_image(lower_left_image_info, hit_bary));
                return;
            }
            // right bottom corner - only top left
            else if (!upper_right_found && !lower_left_found && !upper_right_found && upper_left_found) {
                set_pay_load(single_image(upper_left_image_info, hit_bary));
                return;
            }


            // center above - bottom left and right
            else if (!upper_left_found && !upper_right_found && lower_left_found && lower_right_found) {
                set_pay_load(two_images(lower_left_image_info, lower_left_distance, lower_right_image_info, lower_right_distance, hit_bary));
                return;
            }
            // center below - top left and right
            else if (!lower_left_found && !lower_right_found && upper_left_found && upper_right_found) {
                set_pay_load(two_images(upper_left_image_info, upper_left_distance, upper_right_image_info, upper_right_distance, hit_bary));
                return;
            }
            // center left - right top and bottom
            else if (!upper_left_found && !lower_left_found && upper_right_found && lower_right_found) {
                set_pay_load(two_images(upper_right_image_info, upper_right_distance, lower_right_image_info, lower_right_distance, hit_bary));
                return;
            }
            // center right - left top and bottom
            else if (!upper_right_found && !lower_right_found && upper_left_found && lower_left_found) {
                set_pay_load(two_images(upper_left_image_info, upper_left_distance, lower_left_image_info, lower_left_distance, hit_bary));
                return;
            }
        }
    }

    // set miss values by default
    pay_load.color = vec4(0.1, 0.1, 0.1, 1.0);
    pay_load.distance = -1.0;
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

    // vec3 viewer_point = calculate_orthogonal_point(plane, global_origin.xyz);
    vec3 point = gl_WorldRayOriginNV + gl_WorldRayDirectionNV * gl_HitTNV;

    interpolate_images(plane, info_selector, calculate_barycentrics(plane, point));

    // interpolate_images(
    //     plane,
    //     calculate_barycentrics(plane, point),
    //     calculate_barycentrics(plane, viewer_point)
    // );
}