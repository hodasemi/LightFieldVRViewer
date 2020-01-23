use cgmath::{vec2, vec4, InnerSpace, Matrix4, Vector2, Vector3, Vector4, Zero};
use context::prelude::*;

use super::light_field::light_field_data::LightFieldFrustum;
use super::light_field_viewer::{PlaneImageInfo, PlaneInfo};

use std::f32;
use std::ops::IndexMut;
use std::sync::{Arc, Mutex};

struct LightField {
    frustum: LightFieldFrustum,

    planes: Vec<Plane>,
}

impl LightField {
    pub fn new(
        queue: &Arc<Mutex<Queue>>,
        command_buffer: &Arc<CommandBuffer>,
        data: impl IntoIterator<Item = (PlaneInfo, [Vector3<f32>; 6], Vec<PlaneImageInfo>)>,
        frustum: LightFieldFrustum,
    ) -> VerboseResult<Self> {
        let mut planes = Vec::new();

        for (plane_info, vertices, image_infos) in data.into_iter() {
            planes.push(Plane::new(
                queue,
                command_buffer,
                plane_info,
                image_infos,
                vertices,
            )?);
        }

        Ok(LightField { frustum, planes })
    }
}

struct Plane {
    info: PlaneInfo,
    image_infos: Vec<PlaneImageInfo>,
    vertex_buffer: Arc<Buffer<Vector3<f32>>>,
}

impl Plane {
    fn new(
        queue: &Arc<Mutex<Queue>>,
        command_buffer: &Arc<CommandBuffer>,
        plane_info: PlaneInfo,
        image_infos: Vec<PlaneImageInfo>,
        vertices: [Vector3<f32>; 6],
    ) -> VerboseResult<Self> {
        let cpu_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | VK_MEMORY_PROPERTY_HOST_CACHED_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_RAY_TRACING_BIT_NV | VK_BUFFER_USAGE_TRANSFER_SRC_BIT)
            .set_data(&vertices)
            .build(command_buffer.device().clone())?;

        let gpu_buffer = Buffer::into_device_local(cpu_buffer, command_buffer, queue)?;

        Ok(Plane {
            info: plane_info,
            image_infos,
            vertex_buffer: gpu_buffer,
        })
    }
}

#[derive(Clone)]
pub enum InterpolationResult {
    AccelerationStructures(Vec<Arc<AccelerationStructure>>, Arc<AccelerationStructure>),
    Empty,
}

pub struct CPUInterpolation {
    light_fields: Vec<LightField>,

    last_position: Mutex<Vector3<f32>>,

    last_result: Mutex<InterpolationResult>,
}

impl CPUInterpolation {
    pub fn new(
        queue: &Arc<Mutex<Queue>>,
        command_buffer: &Arc<CommandBuffer>,
        interpolation_infos: Vec<(
            impl IntoIterator<Item = (PlaneInfo, [Vector3<f32>; 6], Vec<PlaneImageInfo>)>,
            LightFieldFrustum,
        )>,
    ) -> VerboseResult<Self> {
        let mut light_fields = Vec::with_capacity(interpolation_infos.len());

        for (data, frustum) in interpolation_infos.into_iter() {
            light_fields.push(LightField::new(queue, command_buffer, data, frustum)?);
        }

        Ok(CPUInterpolation {
            light_fields,
            last_position: Mutex::new(Vector3::new(f32::MAX, f32::MAX, f32::MAX)),
            last_result: Mutex::new(InterpolationResult::Empty),
        })
    }

    pub fn calculate_interpolation(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        context: &Context,
        inv_view: Matrix4<f32>,
        mut plane_infos: impl IndexMut<usize, Output = PlaneInfo>,
    ) -> VerboseResult<InterpolationResult> {
        let my_position = (inv_view * vec4(0.0, 0.0, 0.0, 1.0)).truncate();
        let mut last_position = self.last_position.lock()?;

        if Self::check_pos(*last_position, my_position) {
            return Ok(self.last_result.lock()?.clone());
        }

        *last_position = my_position;

        let mut blasses = Vec::new();
        let mut i = 0;

        for light_field in self.light_fields.iter() {
            let viewer_is_inside = light_field.frustum.check(my_position);

            if !viewer_is_inside {
                continue;
            }

            for plane in light_field.planes.iter() {
                if let Some(viewer_point) = Self::plane_line_intersection(
                    &plane,
                    my_position,
                    -plane.info.normal.truncate(),
                ) {
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

                    let (indices, bary) = if viewer_barycentric.y < 0.0 {
                        // check horizontal axis
                        if viewer_barycentric.x < 0.0 {
                            // Above, Left Side
                            Self::selector_of_one(
                                self.find_closest_bottom_right(plane, vec2(0.0001, 0.0001)),
                                "bottom right",
                            )?
                        } else if viewer_barycentric.x > 1.0 {
                            // Above, Right Side
                            Self::selector_of_one(
                                self.find_closest_bottom_left(plane, vec2(0.9999, 0.0001)),
                                "bottom left",
                            )?
                        } else {
                            // Above Center
                            Self::selector_of_two_x(
                                self.find_closest_bottom_left(
                                    plane,
                                    vec2(viewer_barycentric.x, 0.0001),
                                ),
                                self.find_closest_bottom_right(
                                    plane,
                                    vec2(viewer_barycentric.x, 0.0001),
                                ),
                                vec2(viewer_barycentric.x, 0.0001),
                                "bottom left and bottom right",
                            )?
                        }
                    }
                    // check for below
                    else if viewer_barycentric.y > 1.0 {
                        // check horizontal axis
                        if viewer_barycentric.x < 0.0 {
                            // Below, Left Side
                            Self::selector_of_one(
                                self.find_closest_top_right(plane, vec2(0.0001, 0.9999)),
                                "top right",
                            )?
                        } else if viewer_barycentric.x > 1.0 {
                            // Below, Right Side
                            Self::selector_of_one(
                                self.find_closest_top_left(plane, vec2(0.9999, 0.9999)),
                                "top left",
                            )?
                        } else {
                            // Below Center
                            Self::selector_of_two_x(
                                self.find_closest_top_left(
                                    plane,
                                    vec2(viewer_barycentric.x, 0.9999),
                                ),
                                self.find_closest_top_right(
                                    plane,
                                    vec2(viewer_barycentric.x, 0.9999),
                                ),
                                vec2(viewer_barycentric.x, 0.9999),
                                "top right and top left",
                            )?
                        }
                    }
                    // we are in the center, vertically
                    else {
                        // check horizontal axis
                        if viewer_barycentric.x < 0.0 {
                            // Left Side
                            Self::selector_of_two_y(
                                self.find_closest_top_right(
                                    plane,
                                    vec2(0.0001, viewer_barycentric.y),
                                ),
                                self.find_closest_bottom_right(
                                    plane,
                                    vec2(0.0001, viewer_barycentric.y),
                                ),
                                vec2(0.0001, viewer_barycentric.y),
                                "bottom right and top right",
                            )?
                        } else if viewer_barycentric.x > 1.0 {
                            // Right Side
                            Self::selector_of_two_y(
                                self.find_closest_top_left(
                                    plane,
                                    vec2(0.9999, viewer_barycentric.y),
                                ),
                                self.find_closest_bottom_left(
                                    plane,
                                    vec2(0.9999, viewer_barycentric.y),
                                ),
                                vec2(0.9999, viewer_barycentric.y),
                                "bottom left and top left",
                            )?
                        } else {
                            // We hit the plane
                            Self::selector_of_four(
                                self.find_closest_top_left(plane, viewer_barycentric),
                                self.find_closest_bottom_left(plane, viewer_barycentric),
                                self.find_closest_top_right(plane, viewer_barycentric),
                                self.find_closest_bottom_right(plane, viewer_barycentric),
                                viewer_barycentric,
                            )?
                        }
                    };

                    plane_infos[i] = plane.info.clone(indices, bary);

                    let blas = AccelerationStructure::bottom_level()
                        .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV)
                        .add_vertices(&plane.vertex_buffer, None)
                        .build(context.device().clone())?;

                    blas.generate(command_buffer)?;

                    blasses.push(blas);

                    i += 1;
                }
            }
        }

        // return if haven't added any geometry to the blas
        if i == 0 {
            *self.last_result.lock()? = InterpolationResult::Empty;
            return Ok(InterpolationResult::Empty);
        }

        let mut tlas_builder = AccelerationStructure::top_level()
            .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV);

        for blas in blasses.iter() {
            tlas_builder = tlas_builder.add_instance(blas, None, 0);
        }

        let tlas = tlas_builder.build(context.device().clone())?;

        tlas.generate(command_buffer)?;

        *self.last_result.lock()? =
            InterpolationResult::AccelerationStructures(blasses.clone(), tlas.clone());

        Ok(InterpolationResult::AccelerationStructures(blasses, tlas))
    }

    /// https://en.wikipedia.org/wiki/Line%E2%80%93plane_intersection#Algebraic_form
    fn plane_line_intersection(
        plane: &Plane,
        origin: Vector3<f32>,
        direction: Vector3<f32>,
    ) -> Option<Vector3<f32>> {
        let numerator = (plane.info.top_left.truncate() - origin).dot(plane.info.normal.truncate());
        let denominator = plane.info.normal.truncate().dot(direction);

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

    fn check_pos(p1: Vector3<f32>, p2: Vector3<f32>) -> bool {
        Self::almost_eq(p1.x, p2.x) && Self::almost_eq(p1.y, p2.y) && Self::almost_eq(p1.z, p2.z)
    }

    #[inline]
    fn almost_eq(f1: f32, f2: f32) -> bool {
        (f1 - f2).abs() < 0.0001
    }

    fn calculate_barycentrics(plane: &Plane, point: Vector3<f32>) -> Vector2<f32> {
        let horizontal_direction = plane.info.top_right.truncate() - plane.info.top_left.truncate();
        let vertical_direction = plane.info.bottom_left.truncate() - plane.info.top_left.truncate();

        let x = Self::distance_to_line(plane.info.top_left.truncate(), vertical_direction, point)
            / horizontal_direction.magnitude();

        let y = Self::distance_to_line(plane.info.top_left.truncate(), horizontal_direction, point)
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

    fn find_closest_bottom_right(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff >= 0.0 && y_diff >= 0.0)
    }

    fn find_closest_top_right(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff >= 0.0 && y_diff <= 0.0)
    }

    fn find_closest_bottom_left(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff <= 0.0 && y_diff >= 0.0)
    }

    fn find_closest_top_left(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff <= 0.0 && y_diff <= 0.0)
    }

    #[inline]
    fn find_closest<F>(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
        f: F,
    ) -> Option<(i32, Vector2<f32>)>
    where
        F: Fn(f32, f32) -> bool,
    {
        let mut minimal_distance = f32::MAX;
        let mut info_bary = Vector2::zero();
        let mut image_info_index = None;

        for info in plane.image_infos.iter() {
            let x_diff = info.center.x - bary.x;
            let y_diff = info.center.y - bary.y;

            if f(x_diff, y_diff) {
                let new_distance = vec2(x_diff, y_diff).magnitude();

                if new_distance < minimal_distance {
                    minimal_distance = new_distance;
                    image_info_index = Some(info.image_index);
                    info_bary = info.center;
                }
            }
        }

        match image_info_index {
            Some(index) => Some((index as i32, info_bary)),
            None => None,
        }
    }

    #[inline]
    fn selector_of_one(
        first: Option<(i32, Vector2<f32>)>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>)> {
        match first {
            Some((index, _)) => Ok((vec4(index, -1, -1, -1), vec2(0.0, 0.0))),
            None => create_error!(format!("nothing could be found for {}", debug)),
        }
    }

    #[inline]
    fn selector_of_two_y(
        above: Option<(i32, Vector2<f32>)>,
        below: Option<(i32, Vector2<f32>)>,
        barycentric: Vector2<f32>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>)> {
        match (above, below) {
            // both first and second have been found
            (Some((above_index, above_center)), Some((below_index, below_center))) => {
                assert!(above_center.y < below_center.y);

                let weight = below_center.y - barycentric.y;

                Ok((vec4(above_index, below_index, -1, -1), vec2(weight, 0.0)))
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
    fn selector_of_two_x(
        left: Option<(i32, Vector2<f32>)>,
        right: Option<(i32, Vector2<f32>)>,
        barycentric: Vector2<f32>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>)> {
        match (left, right) {
            // both first and second have been found
            (Some((left_index, left_center)), Some((right_index, right_center))) => {
                assert!(left_center.x < right_center.x);

                let weight = right_center.x - barycentric.x;

                Ok((vec4(left_index, right_index, -1, -1), vec2(weight, 0.0)))
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
        top_left: Option<(i32, Vector2<f32>)>,
        bottom_left: Option<(i32, Vector2<f32>)>,
        top_right: Option<(i32, Vector2<f32>)>,
        bottom_right: Option<(i32, Vector2<f32>)>,
        barycentric: Vector2<f32>,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>)> {
        match (bottom_left, top_left, bottom_right, top_right) {
            // all 4 were found
            (
                Some((bottom_left_index, _)),
                Some((top_left_index, _)),
                Some((bottom_right_index, bottom_right_center)),
                Some((top_right_index, _)),
            ) => {
                let x = bottom_right_center.x - barycentric.x;
                let y = bottom_right_center.y - barycentric.y;

                Ok((vec4(
                        bottom_left_index,
                        top_left_index,
                        bottom_right_index,
                        top_right_index,
                    ),
                    vec2(x,y)
                ))
            }
            // only bottom left and bottom right - center above
            (
                Some((bottom_left_index, bottom_left_center)),
                None,
                Some((bottom_right_index, bottom_right_center)),
                None,
            ) => Self::selector_of_two_x(
                Some((bottom_left_index, bottom_left_center)),
                Some((bottom_right_index, bottom_right_center)),
                barycentric,
                "",
            ),
            // only top left and top right - center below
            (
                None,
                Some((top_left_index, top_left_center)),
                None,
                Some((top_right_index, top_right_center)),
            ) => Self::selector_of_two_x(
                Some((top_left_index, top_left_center)),
                Some((top_right_index, top_right_center)),
                barycentric,
                "",
            ),
            // only top right and bottom right - center left
            (
                None,
                None,
                Some((bottom_right_index, bottom_right_center)),
                Some((top_right_index, top_right_center)),
            ) => Self::selector_of_two_y(
                Some((top_right_index, top_right_center)),
                Some((bottom_right_index, bottom_right_center)),
                barycentric,
                "",
            ),
            // only top left and bottom left - center right
            (
                Some((bottom_left_index, bottom_left_center)),
                Some((top_left_index, top_left_center)),
                None,
                None,
            ) => Self::selector_of_two_y(
                Some((top_left_index, top_left_center)),
                Some((bottom_left_index, bottom_left_center)),
                barycentric,
                "",
            ),
            // only bottom right - left top corner
            (None, None, Some((index, _)), None) => Ok((vec4(index, -1, -1, -1), vec2(0.0, 0.0))),
            // only top right - left bottom corner
            (None, None, None, Some((index, _))) => Ok((vec4(index, -1, -1, -1), vec2(0.0, 0.0))),
            // only bottom left - right top corner
            (Some((index, _)), None, None, None) => Ok((vec4(index, -1, -1, -1), vec2(0.0, 0.0))),
            // only top left - right bottom corner
            (None, Some((index, _)), None, None) => Ok((vec4(index, -1, -1, -1), vec2(0.0, 0.0))),
            _ => create_error!(format!(
                "no fitting constellation found: \n\tbottom_left: {:?}, \n\ttop_left: {:?}, \n\tbottom_right: {:?}, \n\ttop_right: {:?}",
                bottom_left, top_left, bottom_right, top_right
            ))
        }
    }
}
