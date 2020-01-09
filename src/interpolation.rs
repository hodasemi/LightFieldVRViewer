use cgmath::{vec2, vec4, InnerSpace, Matrix4, Vector2, Vector3};
use context::prelude::*;

use super::light_field_viewer::{InfoSelector, PlaneImageInfo, PlaneVertex, MAX_IMAGES_PER_LAYER};

use std::f32;
use std::ops::IndexMut;

struct Plane {
    top_left: Vector3<f32>,
    top_right: Vector3<f32>,
    bottom_left: Vector3<f32>,
    _bottom_right: Vector3<f32>,

    normal: Vector3<f32>,

    first_index: usize,
    last_index: usize,
}

impl Plane {
    fn new(plane_vertices: &[PlaneVertex], index: usize) -> Self {
        let (v0, v1, v2, v5) = if (index % 2) == 0 {
            (
                &plane_vertices[3 * index],
                &plane_vertices[3 * index + 1],
                &plane_vertices[3 * index + 2],
                &plane_vertices[3 * index + 5],
            )
        } else {
            (
                &plane_vertices[3 * index - 3],
                &plane_vertices[3 * index - 2],
                &plane_vertices[3 * index - 1],
                &plane_vertices[3 * index + 2],
            )
        };

        Plane {
            top_left: v1.position_first.xyz(),
            top_right: v5.position_first.xyz(),
            bottom_left: v0.position_first.xyz(),
            _bottom_right: v2.position_first.xyz(),

            normal: v0.normal_last.xyz(),

            first_index: v0.position_first.w().x as usize,
            last_index: v0.normal_last.w().x as usize,
        }
    }
}

pub struct CPUInterpolation {
    planes: Vec<Plane>,
    secondary_data: Vec<PlaneImageInfo>,
}

impl CPUInterpolation {
    pub fn new(plane_vertices: &[PlaneVertex], secondary_data: Vec<PlaneImageInfo>) -> Self {
        let mut planes = Vec::with_capacity(plane_vertices.len() / 6);

        for index in (0..(plane_vertices.len() / 3)).step_by(2) {
            planes.push(Plane::new(plane_vertices, index));
        }

        CPUInterpolation {
            planes,
            secondary_data,
        }
    }

    pub fn calculate_interpolation(
        &self,
        inv_view: Matrix4<f32>,
        mut selector: impl IndexMut<usize, Output = InfoSelector>,
    ) -> VerboseResult<()> {
        let my_position = (inv_view * vec4(0.0, 0.0, 0.0, 1.0)).truncate();

        for (i, plane) in self.planes.iter().enumerate() {
            if let Some(viewer_point) =
                Self::plane_line_intersection(&plane, my_position, -plane.normal)
            {
                let viewer_barycentric = Self::calculate_barycentrics(&plane, viewer_point);

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

                if viewer_barycentric.y < 0.0 {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Above, Left Side
                        selector[i] = self.set_image_infos(plane, vec2(0.0001, 0.0001));
                    } else if viewer_barycentric.x > 1.0 {
                        // Above, Right Side
                        selector[i] = self.set_image_infos(plane, vec2(0.9999, 0.0001));
                    } else {
                        // Above Center
                        selector[i] =
                            self.set_image_infos(plane, vec2(viewer_barycentric.x, 0.0001));
                    }
                }
                // check for below
                else if viewer_barycentric.y > 1.0 {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Below, Left Side
                        selector[i] = self.set_image_infos(plane, vec2(0.0001, 0.9999));
                    } else if viewer_barycentric.x > 1.0 {
                        // Below, Right Side
                        selector[i] = self.set_image_infos(plane, vec2(0.9999, 0.9999));
                    } else {
                        // Below Center
                        selector[i] =
                            self.set_image_infos(plane, vec2(viewer_barycentric.x, 0.9999));
                    }
                }
                // we are in the center, vertically
                else {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Left Side
                        selector[i] =
                            self.set_image_infos(plane, vec2(0.0001, viewer_barycentric.y));
                    } else if viewer_barycentric.x > 1.0 {
                        // Right Side
                        selector[i] =
                            self.set_image_infos(plane, vec2(0.9999, viewer_barycentric.y));
                    } else {
                        // We hit the plane
                        selector[i] = self.set_image_infos(plane, viewer_barycentric);
                    }
                }
            }
        }

        Ok(())
    }

    fn set_image_infos(&self, plane: &Plane, bary: Vector2<f32>) -> InfoSelector {
        let indices = self.find_overlapping_images(plane, bary);
        let weight = 1.0 / indices.len() as f32;

        let mut info_selector = InfoSelector::default();

        for (i, index) in indices.iter().enumerate() {
            info_selector.indices[i] = *index as i32;
            info_selector.weights[i] = weight;
        }

        info_selector
    }

    fn find_overlapping_images(&self, plane: &Plane, bary: Vector2<f32>) -> Vec<u32> {
        let mut image_info_indices = Vec::new();

        for (index, info) in self.secondary_data[plane.first_index..plane.last_index]
            .iter()
            .enumerate()
        {
            if info.check_inside(bary) {
                image_info_indices.push((index + plane.first_index) as u32);
            }
        }

        if image_info_indices.len() > MAX_IMAGES_PER_LAYER {
            panic!("more images found than there are supported");
        }

        image_info_indices
    }

    /// https://en.wikipedia.org/wiki/Line%E2%80%93plane_intersection#Algebraic_form
    fn plane_line_intersection(
        plane: &Plane,
        origin: Vector3<f32>,
        direction: Vector3<f32>,
    ) -> Option<Vector3<f32>> {
        let numerator = (plane.top_left - origin).dot(plane.normal);
        let denominator = plane.normal.dot(direction);

        if denominator == 0.0 {
            // println!("plane and line are parallel");
            // if numerator == 0.0 {
            //     println!("the plane contains the line");
            // } else {
            //     println!("the plane and the line will never intersect");
            // }

            return None;
        }

        if numerator == 0.0 {
            // println!("numerator is zero");

            return None;
        }

        let distance = numerator / denominator;

        Some(origin + (direction * distance))
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

    fn distance_to_line(
        reference: Vector3<f32>,
        normal: Vector3<f32>,
        target: Vector3<f32>,
    ) -> f32 {
        normal.dot(target - reference) / normal.magnitude()
    }

    // fn find_closest_bottom_right(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
    //     self.find_closest(plane, bary, |x_diff, y_diff| {
    //         if x_diff < 0.0 {
    //             return None;
    //         }

    //         if y_diff < 0.0 {
    //             return None;
    //         }

    //         Some(x_diff + y_diff)
    //     })
    // }

    // fn find_closest_top_right(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
    //     self.find_closest(plane, bary, |x_diff, y_diff| {
    //         if x_diff < 0.0 {
    //             return None;
    //         }

    //         if y_diff > 0.0 {
    //             return None;
    //         }

    //         Some(x_diff - y_diff)
    //     })
    // }

    // fn find_closest_bottom_left(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
    //     self.find_closest(plane, bary, |x_diff, y_diff| {
    //         if x_diff > 0.0 {
    //             return None;
    //         }

    //         if y_diff < 0.0 {
    //             return None;
    //         }

    //         Some(y_diff - x_diff)
    //     })
    // }

    // fn find_closest_top_left(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
    //     self.find_closest(plane, bary, |x_diff, y_diff| {
    //         if x_diff > 0.0 {
    //             return None;
    //         }

    //         if y_diff > 0.0 {
    //             return None;
    //         }

    //         Some(-(x_diff + y_diff))
    //     })
    // }

    // #[inline]
    // fn find_closest<F>(&self, plane: &Plane, bary: Vector2<f32>, f: F) -> Option<(i32, f32)>
    // where
    //     F: Fn(f32, f32) -> Option<f32>,
    // {
    //     let mut minimal_distance = f32::MAX;
    //     let mut image_info_index = None;

    //     for (index, info) in self.secondary_data[plane.first_index..plane.last_index]
    //         .iter()
    //         .enumerate()
    //     {
    //         let x_diff = info.center.x - bary.x;
    //         let y_diff = info.center.y - bary.y;

    //         if let Some(new_distance) = f(x_diff, y_diff) {
    //             if new_distance < minimal_distance {
    //                 minimal_distance = new_distance;
    //                 image_info_index = Some(plane.first_index + index);
    //             }
    //         }
    //     }

    //     match image_info_index {
    //         Some(index) => Some((index as i32, minimal_distance)),
    //         None => None,
    //     }
    // }

    // #[inline]
    // fn weight_from_two(first_distance: f32, second_distance: f32) -> (f32, f32) {
    //     let total_distance = first_distance + second_distance;

    //     let first_weight = second_distance / total_distance;
    //     let second_weight = first_distance / total_distance;

    //     (first_weight, second_weight)
    // }

    // #[inline]
    // fn weight_from_four(
    //     first_distance: f32,
    //     second_distance: f32,
    //     third_distance: f32,
    //     fourth_distance: f32,
    // ) -> (f32, f32, f32, f32) {
    //     let total_distance = first_distance + second_distance + third_distance + fourth_distance;

    //     // ------------------------------------------------------------------------------
    //     // ---------------------------------   TODO   -----------------------------------
    //     // ------------------------------------------------------------------------------
    //     let first_weight = first_distance / total_distance;
    //     let second_weight = second_distance / total_distance;
    //     let third_weight = third_distance / total_distance;
    //     let fourth_weight = fourth_distance / total_distance;
    //     // ------------------------------------------------------------------------------
    //     // ---------------------------------   TODO   -----------------------------------
    //     // ------------------------------------------------------------------------------

    //     (first_weight, second_weight, third_weight, fourth_weight)
    // }

    // #[inline]
    // fn selector_of_two(
    //     first: Option<(i32, f32)>,
    //     second: Option<(i32, f32)>,
    //     debug: &str,
    // ) -> VerboseResult<InfoSelector> {
    //     match (first, second) {
    //         // both first and second have been found
    //         (Some((first_index, first_distance)), Some((second_index, second_distance))) => {
    //             let (first_weight, second_weight) =
    //                 Self::weight_from_two(first_distance, second_distance);

    //             Ok(InfoSelector {
    //                 indices: vec4(first_index, second_index, -1, -1),
    //                 weights: vec4(first_weight, second_weight, 0.0, 0.0),
    //             })
    //         }
    //         // only left could be found
    //         (Some((index, _)), None) => Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         // only right could be found
    //         (None, Some((index, _))) => Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         // none could be found
    //         (None, None) => {
    //             create_error!(format!(
    //                 "neither left nor right could be found for {}",
    //                 debug
    //             ));
    //         }
    //     }
    // }

    // #[inline]
    // fn selector_of_four(
    //     top_left: Option<(i32, f32)>,
    //     bottom_left: Option<(i32, f32)>,
    //     top_right: Option<(i32, f32)>,
    //     bottom_right: Option<(i32, f32)>,
    // ) -> VerboseResult<InfoSelector> {
    //     match (bottom_left, top_left, bottom_right, top_right) {
    //         // all 4 were found
    //         (
    //             Some((bottom_left_index, bottom_left_distance)),
    //             Some((top_left_index, top_left_distance)),
    //             Some((bottom_right_index, bottom_right_distance)),
    //             Some((top_right_index, top_right_distance)),
    //         ) => {
    //             let (bottom_left_weight, top_left_weight, bottom_right_weight, top_right_weight) =
    //                 Self::weight_from_four(
    //                     bottom_left_distance,
    //                     top_left_distance,
    //                     bottom_right_distance,
    //                     top_right_distance,
    //                 );

    //             Ok(InfoSelector {
    //                 indices: vec4(
    //                     bottom_left_index,
    //                     top_left_index,
    //                     bottom_right_index,
    //                     top_right_index,
    //                 ),
    //                 weights: vec4(
    //                     bottom_left_weight,
    //                     top_left_weight,
    //                     bottom_right_weight,
    //                     top_right_weight,
    //                 ),
    //             })
    //         }
    //         // only bottom left and bottom right - center above
    //         (
    //             Some((bottom_left_index, bottom_left_distance)),
    //             None,
    //             Some((bottom_right_index, bottom_right_distance)),
    //             None,
    //         ) => Self::selector_of_two(
    //             Some((bottom_left_index, bottom_left_distance)),
    //             Some((bottom_right_index, bottom_right_distance)),
    //             "",
    //         ),
    //         // only top left and top right - center below
    //         (
    //             None,
    //             Some((top_left_index, top_left_distance)),
    //             None,
    //             Some((top_right_index, top_right_distance)),
    //         ) => Self::selector_of_two(
    //             Some((top_left_index, top_left_distance)),
    //             Some((top_right_index, top_right_distance)),
    //             "",
    //         ),
    //         // only top right and bottom right - center left
    //         (
    //             None,
    //             None,
    //             Some((bottom_right_index, bottom_right_distance)),
    //             Some((top_right_index, top_right_distance)),
    //         ) => Self::selector_of_two(
    //             Some((bottom_right_index, bottom_right_distance)),
    //             Some((top_right_index, top_right_distance)),
    //             "",
    //         ),
    //         // only top left and bottom left - center right
    //         (
    //             Some((bottom_left_index, bottom_left_distance)),
    //             Some((top_left_index, top_left_distance)),
    //             None,
    //             None,
    //         ) => Self::selector_of_two(
    //             Some((bottom_left_index, bottom_left_distance)),
    //             Some((top_left_index, top_left_distance)),
    //             "",
    //         ),
    //         // only bottom right - left top corner
    //         (None, None, Some((index, _)), None) =>
    //         Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         // only top right - left bottom corner
    //         (None, None, None, Some((index, _))) => Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         // only bottom left - right top corner
    //         (Some((index, _)), None, None, None) =>  Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         // only top left - right bottom corner
    //         (None, Some((index, _)), None, None) =>  Ok(InfoSelector {
    //             indices: vec4(index, -1, -1, -1),
    //             weights: vec4(1.0, 0.0, 0.0, 0.0),
    //         }),
    //         _ => create_error!(format!("no fitting constellation found: bottom_left: {:?}, top_left: {:?}, bottom_right: {:?}, top_right: {:?}", bottom_left, top_left, bottom_right, top_right))
    //     }
    // }
}
