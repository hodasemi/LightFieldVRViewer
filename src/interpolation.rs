use cgmath::{vec2, vec4, InnerSpace, Matrix4, Vector2, Vector3, Vector4};
use context::prelude::*;

use super::light_field::light_field_data::LightFieldFrustum;
use super::light_field_viewer::{PlaneImageInfo, PlaneInfo};

use ordered_float::OrderedFloat;
use std::collections::BinaryHeap;
use std::f32;
use std::ops::IndexMut;

#[derive(Eq, Debug)]
struct IndexedFloat {
    index: u32,
    value: OrderedFloat<f32>,
}

impl IndexedFloat {
    fn new(index: u32, value: f32) -> Self {
        IndexedFloat {
            index,
            value: OrderedFloat(value),
        }
    }

    fn index(&self) -> usize {
        self.index as usize
    }

    fn value(&self) -> f32 {
        self.value.0
    }
}

impl Ord for IndexedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for IndexedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for IndexedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

struct Plane {
    top_left: Vector3<f32>,
    top_right: Vector3<f32>,
    bottom_left: Vector3<f32>,
    _bottom_right: Vector3<f32>,

    normal: Vector3<f32>,

    image_infos: Vec<PlaneImageInfo>,

    frustum: LightFieldFrustum,
}

impl Plane {
    fn new(
        plane_info: PlaneInfo,
        frustum: LightFieldFrustum,
        image_infos: Vec<PlaneImageInfo>,
    ) -> Self {
        Plane {
            top_left: plane_info.top_left.truncate(),
            top_right: plane_info.top_right.truncate(),
            bottom_left: plane_info.bottom_left.truncate(),
            _bottom_right: plane_info.bottom_right.truncate(),

            normal: plane_info.normal.truncate(),

            image_infos,

            frustum,
        }
    }
}

pub struct CPUInterpolation {
    planes: Vec<Plane>,
}

impl CPUInterpolation {
    pub fn new(
        interpolation_infos: Vec<(PlaneInfo, LightFieldFrustum, Vec<PlaneImageInfo>)>,
    ) -> Self {
        let mut planes = Vec::with_capacity(interpolation_infos.len());

        for (plane_info, frustum, image_infos) in interpolation_infos.iter() {
            planes.push(Plane::new(
                plane_info.clone(),
                frustum.clone(),
                image_infos.clone(),
            ));
        }

        CPUInterpolation { planes }
    }

    pub fn calculate_interpolation(
        &self,
        context: &Context,
        inv_view: Matrix4<f32>,
        mut selector: impl IndexMut<usize, Output = PlaneInfo>,
    ) -> VerboseResult<()> {
        let my_position = (inv_view * vec4(0.0, 0.0, 0.0, 1.0)).truncate();

        let mut found_one = false;

        for (i, plane) in self.planes.iter().enumerate() {
            if let Some(viewer_point) =
                Self::plane_line_intersection(&plane, my_position, -plane.normal)
            {
                let viewer_barycentric = Self::calculate_barycentrics(&plane, viewer_point);

                let viewer_is_inside = plane.frustum.check(viewer_point);

                if viewer_is_inside {
                    // selector[i].indices = vec4(-1, -1, -1, -1);
                    // selector[i].weights = vec4(0.0, 0.0, 0.0, 0.0);

                    found_one = true;

                    // continue;
                }

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

                // if viewer_barycentric.x <= 1.0
                //     && viewer_barycentric.x >= 0.0
                //     && viewer_barycentric.y <= 1.0
                //     && viewer_barycentric.y >= 0.0
                // {
                //     // We hit the plane
                //     selector[i] = Self::selector_of_four(
                //         self.find_closest_top_left(plane, viewer_barycentric),
                //         self.find_closest_bottom_left(plane, viewer_barycentric),
                //         self.find_closest_top_right(plane, viewer_barycentric),
                //         self.find_closest_bottom_right(plane, viewer_barycentric),
                //     )?;
                // } else {
                //     selector[i] = InfoSelector::default();
                // }

                if viewer_barycentric.y < 0.0 {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Above, Left Side
                        let (indices, weights) = Self::selector_of_one(
                            self.find_closest_bottom_right(plane, vec2(0.0001, 0.0001)),
                            "bottom right",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else if viewer_barycentric.x > 1.0 {
                        // Above, Right Side
                        let (indices, weights) = Self::selector_of_one(
                            self.find_closest_bottom_left(plane, vec2(0.9999, 0.0001)),
                            "bottom left",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else {
                        // Above Center
                        let (indices, weights) = Self::selector_of_two(
                            self.find_closest_bottom_left(
                                plane,
                                vec2(viewer_barycentric.x, 0.0001),
                            ),
                            self.find_closest_bottom_right(
                                plane,
                                vec2(viewer_barycentric.x, 0.0001),
                            ),
                            "bottom left and bottom right",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    }
                }
                // check for below
                else if viewer_barycentric.y > 1.0 {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Below, Left Side
                        let (indices, weights) = Self::selector_of_one(
                            self.find_closest_top_right(plane, vec2(0.0001, 0.9999)),
                            "top right",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else if viewer_barycentric.x > 1.0 {
                        // Below, Right Side
                        let (indices, weights) = Self::selector_of_one(
                            self.find_closest_top_left(plane, vec2(0.9999, 0.9999)),
                            "top left",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else {
                        // Below Center
                        let (indices, weights) = Self::selector_of_two(
                            self.find_closest_top_right(plane, vec2(viewer_barycentric.x, 0.9999)),
                            self.find_closest_top_left(plane, vec2(viewer_barycentric.x, 0.9999)),
                            "top right and top left",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    }
                }
                // we are in the center, vertically
                else {
                    // check horizontal axis
                    if viewer_barycentric.x < 0.0 {
                        // Left Side
                        let (indices, weights) = Self::selector_of_two(
                            self.find_closest_bottom_right(
                                plane,
                                vec2(0.0001, viewer_barycentric.y),
                            ),
                            self.find_closest_top_right(plane, vec2(0.0001, viewer_barycentric.y)),
                            "bottom right and top right",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else if viewer_barycentric.x > 1.0 {
                        // Right Side
                        let (indices, weights) = Self::selector_of_two(
                            self.find_closest_bottom_left(
                                plane,
                                vec2(0.9999, viewer_barycentric.y),
                            ),
                            self.find_closest_top_left(plane, vec2(0.9999, viewer_barycentric.y)),
                            "bottom left and top left",
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    } else {
                        // We hit the plane
                        let (indices, weights) = Self::selector_of_four(
                            self.find_closest_top_left(plane, viewer_barycentric),
                            self.find_closest_bottom_left(plane, viewer_barycentric),
                            self.find_closest_top_right(plane, viewer_barycentric),
                            self.find_closest_bottom_right(plane, viewer_barycentric),
                        )?;

                        selector[i].indices = indices;
                        selector[i].weights = weights;
                    }
                }
            }
        }

        if !found_one {
            // context
            //     .render_core()
            //     .set_clear_color([1.0, 0.2, 0.2, 1.0])?;
        }

        Ok(())
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

    fn find_closest_bottom_right(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
        self.find_closest(plane, bary, |x_diff, y_diff| {
            if x_diff < 0.0 {
                return None;
            }

            if y_diff < 0.0 {
                return None;
            }

            Some(x_diff + y_diff)
        })
    }

    fn find_closest_top_right(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
        self.find_closest(plane, bary, |x_diff, y_diff| {
            if x_diff < 0.0 {
                return None;
            }

            if y_diff > 0.0 {
                return None;
            }

            Some(x_diff - y_diff)
        })
    }

    fn find_closest_bottom_left(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
        self.find_closest(plane, bary, |x_diff, y_diff| {
            if x_diff > 0.0 {
                return None;
            }

            if y_diff < 0.0 {
                return None;
            }

            Some(y_diff - x_diff)
        })
    }

    fn find_closest_top_left(&self, plane: &Plane, bary: Vector2<f32>) -> Option<(i32, f32)> {
        self.find_closest(plane, bary, |x_diff, y_diff| {
            if x_diff > 0.0 {
                return None;
            }

            if y_diff > 0.0 {
                return None;
            }

            Some(-(x_diff + y_diff))
        })
    }

    #[inline]
    fn find_closest<F>(&self, plane: &Plane, bary: Vector2<f32>, f: F) -> Option<(i32, f32)>
    where
        F: Fn(f32, f32) -> Option<f32>,
    {
        let mut minimal_distance = f32::MAX;
        let mut image_info_index = None;

        for info in plane.image_infos.iter() {
            let x_diff = info.center.x - bary.x;
            let y_diff = info.center.y - bary.y;

            if let Some(new_distance) = f(x_diff, y_diff) {
                if new_distance < minimal_distance {
                    minimal_distance = new_distance;
                    image_info_index = Some(info.image_index);
                }
            }
        }

        match image_info_index {
            Some(index) => Some((index as i32, minimal_distance)),
            None => None,
        }
    }

    #[inline]
    fn weight_from_two(first_distance: f32, second_distance: f32) -> (f32, f32) {
        let total_distance = first_distance + second_distance;

        let first_weight = second_distance / total_distance;
        let second_weight = first_distance / total_distance;

        (first_weight, second_weight)
    }

    fn distance_weighting<'a>(
        distances: impl ExactSizeIterator<Item = &'a f32> + Clone,
    ) -> Vec<f32> {
        let amount = distances.len();

        let total_distance: f32 = distances.clone().sum();

        let inv_weights: Vec<f32> = distances
            .map(|distance| distance / total_distance)
            .collect();

        let weights: Vec<f32> = inv_weights
            .iter()
            .map(|inverted_weight| inverted_weight / (1.0 / amount as f32))
            .collect();

        let weight_sum: f32 = weights.iter().sum();

        let normalized_weights: Vec<f32> =
            weights.iter().map(|weight| weight / weight_sum).collect();

        normalized_weights.iter().cloned().rev().collect()
    }

    #[inline]
    fn weight_from_four(
        first_distance: f32,
        second_distance: f32,
        third_distance: f32,
        fourth_distance: f32,
    ) -> (f32, f32, f32, f32) {
        let output_weights = Self::distance_weighting(
            vec![
                first_distance,
                second_distance,
                third_distance,
                fourth_distance,
            ]
            .iter(),
        );

        let sum: f32 = output_weights.iter().sum();

        if (sum - 1.0).abs() > 0.0001 {
            panic!("sum not equal 1.0: sum = {}", sum);
        }

        (
            output_weights[0],
            output_weights[1],
            output_weights[2],
            output_weights[2],
        )
    }

    #[inline]
    fn selector_of_one(
        first: Option<(i32, f32)>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector4<f32>)> {
        match first {
            Some((index, _)) => Ok((vec4(index, -1, -1, -1), vec4(1.0, 0.0, 0.0, 0.0))),
            None => create_error!(format!("nothing could be found for {}", debug)),
        }
    }

    #[inline]
    fn selector_of_two(
        first: Option<(i32, f32)>,
        second: Option<(i32, f32)>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector4<f32>)> {
        match (first, second) {
            // both first and second have been found
            (Some((first_index, first_distance)), Some((second_index, second_distance))) => {
                let (first_weight, second_weight) =
                    Self::weight_from_two(first_distance, second_distance);

                Ok((
                    vec4(first_index, second_index, -1, -1),
                    vec4(first_weight, second_weight, 0.0, 0.0),
                ))
            }
            // only left could be found
            (Some((index, weight)), None) => Self::selector_of_one(Some((index, weight)), ""),
            // only right could be found
            (None, Some((index, weight))) => Self::selector_of_one(Some((index, weight)), ""),
            // none could be found
            (None, None) => {
                create_error!(format!("nothing could be found for {}", debug));
            }
        }
    }

    #[inline]
    fn selector_of_four(
        top_left: Option<(i32, f32)>,
        bottom_left: Option<(i32, f32)>,
        top_right: Option<(i32, f32)>,
        bottom_right: Option<(i32, f32)>,
    ) -> VerboseResult<(Vector4<i32>, Vector4<f32>)> {
        match (bottom_left, top_left, bottom_right, top_right) {
            // all 4 were found
            (
                Some((bottom_left_index, bottom_left_distance)),
                Some((top_left_index, top_left_distance)),
                Some((bottom_right_index, bottom_right_distance)),
                Some((top_right_index, top_right_distance)),
            ) => {
                let (bottom_left_weight, top_left_weight, bottom_right_weight, top_right_weight) =
                    Self::weight_from_four(
                        bottom_left_distance,
                        top_left_distance,
                        bottom_right_distance,
                        top_right_distance,
                    );

                Ok((vec4(
                        bottom_left_index,
                        top_left_index,
                        bottom_right_index,
                        top_right_index,
                    ),
                    vec4(
                        bottom_left_weight,
                        top_left_weight,
                        bottom_right_weight,
                        top_right_weight,
                    )
                ))
            }
            // only bottom left and bottom right - center above
            (
                Some((bottom_left_index, bottom_left_distance)),
                None,
                Some((bottom_right_index, bottom_right_distance)),
                None,
            ) => Self::selector_of_two(
                Some((bottom_left_index, bottom_left_distance)),
                Some((bottom_right_index, bottom_right_distance)),
                "",
            ),
            // only top left and top right - center below
            (
                None,
                Some((top_left_index, top_left_distance)),
                None,
                Some((top_right_index, top_right_distance)),
            ) => Self::selector_of_two(
                Some((top_left_index, top_left_distance)),
                Some((top_right_index, top_right_distance)),
                "",
            ),
            // only top right and bottom right - center left
            (
                None,
                None,
                Some((bottom_right_index, bottom_right_distance)),
                Some((top_right_index, top_right_distance)),
            ) => Self::selector_of_two(
                Some((bottom_right_index, bottom_right_distance)),
                Some((top_right_index, top_right_distance)),
                "",
            ),
            // only top left and bottom left - center right
            (
                Some((bottom_left_index, bottom_left_distance)),
                Some((top_left_index, top_left_distance)),
                None,
                None,
            ) => Self::selector_of_two(
                Some((bottom_left_index, bottom_left_distance)),
                Some((top_left_index, top_left_distance)),
                "",
            ),
            // only bottom right - left top corner
            (None, None, Some((index, _)), None) => Ok((vec4(index, -1, -1, -1), vec4(1.0, 0.0, 0.0, 0.0))),
            // only top right - left bottom corner
            (None, None, None, Some((index, _))) => Ok((vec4(index, -1, -1, -1), vec4(1.0, 0.0, 0.0, 0.0))),
            // only bottom left - right top corner
            (Some((index, _)), None, None, None) => Ok((vec4(index, -1, -1, -1), vec4(1.0, 0.0, 0.0, 0.0))),
            // only top left - right bottom corner
            (None, Some((index, _)), None, None) => Ok((vec4(index, -1, -1, -1), vec4(1.0, 0.0, 0.0, 0.0))),
            _ => create_error!(format!("no fitting constellation found: \n\tbottom_left: {:?}, \n\ttop_left: {:?}, \n\tbottom_right: {:?}, \n\ttop_right: {:?}", bottom_left, top_left, bottom_right, top_right))
        }
    }
}
