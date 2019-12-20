use context::prelude::*;

use cgmath::{vec2, vec3, InnerSpace, Vector2, Vector3};
use rand::Rng;

use std::sync::Arc;

use super::light_field_viewer::{PlaneImageInfo, PlaneVertex};

pub struct RayTraceDebugger {
    primary_data: Vec<PlaneVertex>,
    secondary_data: Vec<PlaneImageInfo>,
    images: Vec<Arc<Image>>,
}

impl RayTraceDebugger {
    pub fn new(
        primary_data: Vec<PlaneVertex>,
        secondary_data: Vec<PlaneImageInfo>,
        images: Vec<Arc<Image>>,
    ) -> Self {
        RayTraceDebugger {
            primary_data,
            secondary_data,
            images,
        }
    }

    pub fn debug(&self) -> VerboseResult<()> {
        let mut rng = rand::thread_rng();

        // a primitive contains 3 vertices
        // let primitive_id = rng.gen_range(0, self.primary_data.len() / 3);

        let primitive_id = 5;

        let origin = vec3(0.0, 4.3, -2.5);
        let ray_direction = vec3(1.0, -0.015, 0.02);

        println!("primitive id: {}", primitive_id);

        let plane = self.get_plane(primitive_id);

        let point = Self::plane_line_intersection(&plane, origin, ray_direction);
        let viewer_point = Self::plane_line_intersection(&plane, origin, -plane.normal);

        self.interpolate_images(
            &plane,
            Self::calculate_barycentrics(&plane, point),
            Self::calculate_barycentrics(&plane, viewer_point),
        )?;

        Ok(())
    }

    fn interpolate_images(
        &self,
        plane: &Plane,
        hit_bary: Vector2<f32>,
        pov_bary: Vector2<f32>,
    ) -> VerboseResult<()> {
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

        if pov_bary.y < 0.0 {
            // check horizontal axis
            if pov_bary.x < 0.0 {
                // Above, Left Side
                println!("above, left side");

                let image_info = self
                    .find_closest_bottom_right(plane.first_index, plane.last_index, pov_bary)
                    .ok_or("no plane image info found in bottom right")?;

                if !image_info.check_inside(hit_bary) {
                    println!("not inside the image's area");
                }
            } else if pov_bary.x > 1.0 {
                // Above, Right Side
                println!("above, right side");
            } else {
                // Above Center
                println!("above, center");
            }
        }
        // check for below
        else if pov_bary.y > 1.0 {
            // check horizontal axis
            if pov_bary.x < 0.0 {
                // Below, Left Side
                println!("below, center");
            } else if pov_bary.x > 1.0 {
                // Below, Right Side
                println!("below, center");
            } else {
                // Below Center
                println!("below, center");
            }
        }
        // we are in the center, vertically
        else {
            // check horizontal axis
            if pov_bary.x < 0.0 {
                // Left Side
                println!("left side");
            } else if pov_bary.x > 1.0 {
                // Right Side
                println!("right side");
            } else {
                // We hit the plane
                println!("hit the plane");
            }
        }

        Ok(())
    }

    fn find_closest_bottom_right(
        &self,
        start_index: usize,
        end_index: usize,
        bary: Vector2<f32>,
    ) -> Option<PlaneImageInfo> {
        let mut minimal_distance = 2.0;
        let mut image_info = None;

        for info in self.secondary_data[start_index..end_index].iter() {
            let x_diff = info.center.x - bary.x;

            if x_diff < 0.0 {
                continue;
            }

            let y_diff = info.center.y - bary.y;

            if y_diff < 0.0 {
                continue;
            }

            let new_distance = x_diff + y_diff;

            if new_distance < minimal_distance {
                minimal_distance = new_distance;
                image_info = Some(info.clone());
            }
        }

        image_info
    }

    fn get_plane(&self, primitive_id: usize) -> Plane {
        let (v0, v1, v2, v5) = if (primitive_id % 2) == 0 {
            (
                &self.primary_data[3 * primitive_id],
                &self.primary_data[3 * primitive_id + 1],
                &self.primary_data[3 * primitive_id + 2],
                &self.primary_data[3 * primitive_id + 5],
            )
        } else {
            (
                &self.primary_data[3 * primitive_id - 3],
                &self.primary_data[3 * primitive_id - 2],
                &self.primary_data[3 * primitive_id - 1],
                &self.primary_data[3 * primitive_id + 2],
            )
        };

        Plane {
            top_left: v1.position_first.xyz(),
            top_right: v5.position_first.xyz(),
            bottom_left: v0.position_first.xyz(),
            bottom_right: v2.position_first.xyz(),

            normal: v0.normal_last.xyz(),

            first_index: v0.position_first.w().x as usize,
            last_index: v0.normal_last.w().x as usize,
        }
    }

    fn distance_to_line(
        reference: Vector3<f32>,
        normal: Vector3<f32>,
        target: Vector3<f32>,
    ) -> f32 {
        normal.dot(target - reference) / normal.magnitude()
    }

    fn plane_line_intersection(
        plane: &Plane,
        origin: Vector3<f32>,
        direction: Vector3<f32>,
    ) -> Vector3<f32> {
        let numerator = (plane.top_left - origin).dot(plane.normal);
        let denominator = plane.normal.dot(direction);

        if denominator == 0.0 {
            panic!();
        }

        let distance = numerator / denominator;

        origin + (direction * distance)
    }

    fn calculate_barycentrics(plane: &Plane, point: Vector3<f32>) -> Vector2<f32> {
        let horizontal_direction = plane.top_right - plane.top_left;
        let vertical_direction = plane.bottom_left - plane.top_left;

        let x = Self::distance_to_line(plane.top_left, vertical_direction, point)
            / horizontal_direction.magnitude();

        let y = Self::distance_to_line(plane.top_left, horizontal_direction, point)
            / vertical_direction.magnitude();

        vec2(x, y)
    }
}

struct Plane {
    top_left: Vector3<f32>,
    top_right: Vector3<f32>,
    bottom_left: Vector3<f32>,
    bottom_right: Vector3<f32>,

    normal: Vector3<f32>,

    first_index: usize,
    last_index: usize,
}
