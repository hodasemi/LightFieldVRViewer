use cgmath::{vec2, vec4, InnerSpace, Matrix4, Vector2, Vector3, Vector4, Zero};
use context::prelude::*;

use super::light_field::light_field_data::{LightFieldFrustum, PlaneImageRatios};
use super::light_field_viewer::{PlaneImageInfo, PlaneInfo, DEFAULT_FORWARD};

use std::f32;
use std::iter::IntoIterator;
use std::ops::IndexMut;
use std::sync::{Arc, Mutex};

pub struct LightField {
    pub frustum: LightFieldFrustum,
    pub direction: Vector3<f32>,

    pub planes: Vec<Plane>,
}

impl LightField {
    fn new(
        queue: &Arc<Mutex<Queue>>,
        command_buffer: &Arc<CommandBuffer>,
        data: impl IntoIterator<Item = (PlaneInfo, [Vector3<f32>; 6], Vec<PlaneImageInfo>)>,
        frustum: LightFieldFrustum,
        direction: Vector3<f32>,
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

        Ok(LightField {
            frustum,
            planes,
            direction,
        })
    }
}

pub struct Plane {
    pub info: PlaneInfo,
    pub image_infos: Vec<PlaneImageInfo>,
    pub vertex_buffer: Arc<Buffer<Vector3<f32>>>,
    pub blas: Arc<AccelerationStructure>,
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

        let (gpu_buffer, blas) = SingleSubmit::builder(command_buffer, queue, |command_buffer| {
            let gpu_buffer = cpu_buffer.into_device_local(
                command_buffer,
                VK_ACCESS_MEMORY_READ_BIT,
                VK_PIPELINE_STAGE_ACCELERATION_STRUCTURE_BUILD_BIT_NV,
            )?;

            let blas = AccelerationStructure::bottom_level()
                .add_vertices(&gpu_buffer, None)
                .build(command_buffer.device().clone())?;

            blas.generate(command_buffer)?;

            Ok((gpu_buffer, blas))
        })
        .submit()?;

        Ok(Plane {
            info: plane_info,
            image_infos,
            vertex_buffer: gpu_buffer,
            blas,
        })
    }
}

pub struct CPUInterpolation {
    light_fields: Vec<LightField>,

    last_position: TargetMode<Mutex<Vector3<f32>>>,

    last_result: TargetMode<Mutex<Option<Arc<AccelerationStructure>>>>,
}

impl CPUInterpolation {
    pub fn new<T>(
        queue: &Arc<Mutex<Queue>>,
        command_buffer: &Arc<CommandBuffer>,
        interpolation_infos: Vec<(
            impl IntoIterator<Item = (PlaneInfo, [Vector3<f32>; 6], Vec<PlaneImageInfo>)>,
            LightFieldFrustum,
            Vector3<f32>,
        )>,
        mode: TargetMode<T>,
    ) -> VerboseResult<Self> {
        let mut light_fields = Vec::with_capacity(interpolation_infos.len());

        for (data, frustum, direction) in interpolation_infos.into_iter() {
            light_fields.push(LightField::new(
                queue,
                command_buffer,
                data,
                frustum,
                direction,
            )?);
        }

        let last_position = match mode {
            TargetMode::Single(_) => {
                TargetMode::Single(Mutex::new(Vector3::new(f32::MAX, f32::MAX, f32::MAX)))
            }
            TargetMode::Stereo(_, _) => TargetMode::Stereo(
                Mutex::new(Vector3::new(f32::MAX, f32::MAX, f32::MAX)),
                Mutex::new(Vector3::new(f32::MAX, f32::MAX, f32::MAX)),
            ),
        };

        let last_result = match mode {
            TargetMode::Single(_) => TargetMode::Single(Mutex::new(None)),
            TargetMode::Stereo(_, _) => TargetMode::Stereo(Mutex::new(None), Mutex::new(None)),
        };

        Ok(CPUInterpolation {
            light_fields,
            last_position,
            last_result,
        })
    }

    pub fn interpolation(&self) -> TargetMode<Interpolation<'_>> {
        match (&self.last_position, &self.last_result) {
            (TargetMode::Single(last_position), TargetMode::Single(last_result)) => {
                TargetMode::Single(Interpolation {
                    light_fields: &self.light_fields,
                    last_position: last_position,
                    last_result: last_result,
                })
            }
            (
                TargetMode::Stereo(left_last_position, right_last_position),
                TargetMode::Stereo(left_last_result, right_last_result),
            ) => TargetMode::Stereo(
                Interpolation {
                    light_fields: &self.light_fields,
                    last_position: left_last_position,
                    last_result: left_last_result,
                },
                Interpolation {
                    light_fields: &self.light_fields,
                    last_position: right_last_position,
                    last_result: right_last_result,
                },
            ),
            _ => unreachable!(),
        }
    }
}

pub struct Interpolation<'a> {
    pub light_fields: &'a [LightField],
    last_position: &'a Mutex<Vector3<f32>>,
    last_result: &'a Mutex<Option<Arc<AccelerationStructure>>>,
}

impl<'a> Interpolation<'a> {
    pub fn calculate_interpolation(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        context: &Context,
        inv_view: Matrix4<f32>,
        inv_proj: Matrix4<f32>,
        mut plane_infos: impl IndexMut<usize, Output = PlaneInfo>,
    ) -> VerboseResult<Option<Arc<AccelerationStructure>>> {
        let my_position = (inv_view * vec4(0.0, 0.0, 0.0, 1.0)).truncate();

        let mut last_position = self.last_position.lock()?;

        if Self::check_pos(*last_position, my_position) {
            return Ok(self.last_result.lock()?.clone());
        }

        *last_position = my_position;

        let direction = inv_view * (inv_proj * DEFAULT_FORWARD.extend(1.0)).xyz().extend(1.0);
        let light_fields = self.gather_light_fields(my_position, direction.xyz().normalize());

        let mut tlas_builder = AccelerationStructure::top_level()
            .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV);

        let mut i = 0;

        for (light_field_weight, light_field) in light_fields.iter() {
            for plane in light_field.planes.iter() {
                if let Some(intersection_point) = Self::plane_line_intersection(
                    &plane,
                    my_position,
                    -plane.info.normal.truncate(),
                ) {
                    let viewer_barycentric =
                        Self::calculate_barycentrics(&plane, intersection_point);

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

                    let (indices, bary, bounds) = if viewer_barycentric.y < 0.0 {
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

                    plane_infos[i] = plane.info.clone(indices, bary, *light_field_weight, bounds);

                    i += 1;

                    tlas_builder = tlas_builder.add_instance(&plane.blas, None, 0);
                }
            }
        }

        // return if haven't added any geometry to the blas
        if i == 0 {
            *self.last_result.lock()? = None;
            return Ok(None);
        }

        let tlas = tlas_builder.build(context.device().clone())?;

        tlas.generate(command_buffer)?;

        *self.last_result.lock()? = Some(tlas.clone());

        Ok(Some(tlas))
    }

    #[inline]
    fn gather_light_fields(
        &self,
        position: Vector3<f32>,
        direction: Vector3<f32>,
    ) -> Vec<(f32, &LightField)> {
        let mut inside_frustum = Vec::new();

        // look for light fields where we are inside of
        for light_field in self.light_fields.iter() {
            if light_field.frustum.check(position) {
                inside_frustum.push(light_field);
            }
        }

        if inside_frustum.is_empty() {
            let sorted_fields = Self::sort_by_angle(self.light_fields, direction);
            Self::select_and_weight(&sorted_fields)
        } else if inside_frustum.len() == 1 {
            inside_frustum
                .iter()
                .map(|light_field| (1.0, *light_field))
                .collect()
        } else {
            let sorted_fields = Self::sort_by_angle(inside_frustum, direction);
            Self::select_and_weight(&sorted_fields)
        }
    }

    #[inline]
    fn select_and_weight<'c>(light_fields: &[(f32, &'c LightField)]) -> Vec<(f32, &'c LightField)> {
        if light_fields.len() == 1 {
            return light_fields
                .iter()
                .map(|(_, light_field)| (1.0, *light_field))
                .collect();
        }

        let (first_angle, first_light_field) = light_fields[0];
        let (second_angle, second_light_field) = light_fields[1];

        let total = first_angle + second_angle;

        vec![
            (second_angle / total, first_light_field),
            (first_angle / total, second_light_field),
        ]
    }

    #[inline]
    fn sort_by_angle<'c>(
        light_fields: impl IntoIterator<Item = &'c LightField>,
        direction: Vector3<f32>,
    ) -> Vec<(f32, &'c LightField)> {
        let mut fields = Vec::new();

        for light_field in light_fields.into_iter() {
            fields.push((direction.dot(light_field.direction), light_field));
        }

        fields.sort_by(|(left_angle, _), (right_angle, _)| {
            left_angle
                .partial_cmp(right_angle)
                .expect("failed comparing floats")
        });

        fields
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

        vec2(y, x)
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
    ) -> Option<(i32, Vector2<f32>, PlaneImageRatios)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff >= 0.0 && y_diff >= 0.0)
    }

    fn find_closest_top_right(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>, PlaneImageRatios)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff >= 0.0 && y_diff <= 0.0)
    }

    fn find_closest_bottom_left(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>, PlaneImageRatios)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff <= 0.0 && y_diff >= 0.0)
    }

    fn find_closest_top_left(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
    ) -> Option<(i32, Vector2<f32>, PlaneImageRatios)> {
        self.find_closest(plane, bary, |x_diff, y_diff| x_diff <= 0.0 && y_diff <= 0.0)
    }

    #[inline]
    fn find_closest<F>(
        &self,
        plane: &Plane,
        bary: Vector2<f32>,
        f: F,
    ) -> Option<(i32, Vector2<f32>, PlaneImageRatios)>
    where
        F: Fn(f32, f32) -> bool,
    {
        let mut minimal_distance = f32::MAX;
        let mut info_bary = Vector2::zero();
        let mut image_info_index = None;
        let mut bound = None;

        for info in plane.image_infos.iter() {
            let x_diff = info.center.x - bary.x;
            let y_diff = info.center.y - bary.y;

            if f(x_diff, y_diff) {
                let new_distance = vec2(x_diff, y_diff).magnitude();

                if new_distance < minimal_distance {
                    minimal_distance = new_distance;
                    image_info_index = Some(info.image_index);
                    info_bary = info.center;
                    bound = Some(info.ratios.clone());
                }
            }
        }

        match (image_info_index, bound) {
            (Some(index), Some(bound)) => Some((index as i32, info_bary, bound)),
            _ => None,
        }
    }

    #[inline]
    fn selector_of_one(
        first: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>, [PlaneImageRatios; 4])> {
        match first {
            Some((index, _, ratio)) => Ok((
                vec4(index, -1, -1, -1),
                vec2(0.0, 0.0),
                Self::single_ratio(ratio),
            )),
            None => create_error!(format!("nothing could be found for {}", debug)),
        }
    }

    #[inline]
    fn selector_of_two_y(
        above: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        below: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        barycentric: Vector2<f32>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>, [PlaneImageRatios; 4])> {
        match (above, below) {
            // both first and second have been found
            (
                Some((above_index, above_center, above_ratio)),
                Some((below_index, below_center, below_ratio)),
            ) => {
                assert!(above_center.y < below_center.y);

                let weight = below_center.y - barycentric.y;

                Ok((
                    vec4(above_index, below_index, -1, -1),
                    vec2(weight, 0.0),
                    Self::two_ratios(above_ratio, below_ratio),
                ))
            }
            // only above could be found
            (Some((index, weight, ratio)), None) => {
                Self::selector_of_one(Some((index, weight, ratio)), "")
            }
            // only above could be found
            (None, Some((index, weight, ratio))) => {
                Self::selector_of_one(Some((index, weight, ratio)), "")
            }
            // none could be found
            (None, None) => {
                create_error!(format!("nothing could be found for {}", debug));
            }
        }
    }

    #[inline]
    fn selector_of_two_x(
        left: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        right: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        barycentric: Vector2<f32>,
        debug: &str,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>, [PlaneImageRatios; 4])> {
        match (left, right) {
            // both first and second have been found
            (
                Some((left_index, left_center, left_ratio)),
                Some((right_index, right_center, right_ratio)),
            ) => {
                assert!(left_center.x < right_center.x);

                let weight = right_center.x - barycentric.x;

                Ok((
                    vec4(left_index, right_index, -1, -1),
                    vec2(weight, 0.0),
                    Self::two_ratios(left_ratio, right_ratio),
                ))
            }
            // only left could be found
            (Some((index, weight, ratio)), None) => {
                Self::selector_of_one(Some((index, weight, ratio)), "")
            }
            // only right could be found
            (None, Some((index, weight, ratio))) => {
                Self::selector_of_one(Some((index, weight, ratio)), "")
            }
            // none could be found
            (None, None) => {
                create_error!(format!("nothing could be found for {}", debug));
            }
        }
    }

    #[inline]
    fn selector_of_four(
        top_left: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        bottom_left: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        top_right: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        bottom_right: Option<(i32, Vector2<f32>, PlaneImageRatios)>,
        barycentric: Vector2<f32>,
    ) -> VerboseResult<(Vector4<i32>, Vector2<f32>, [PlaneImageRatios; 4])> {
        match (bottom_left, top_left, bottom_right, top_right) {
            // all 4 were found
            (
                Some((bottom_left_index, _, bottom_left_ratio)),
                Some((top_left_index, _, top_left_ratio)),
                Some((bottom_right_index, bottom_right_center, bottom_right_ratio)),
                Some((top_right_index, _, top_right_ratio)),
            ) => {
                let x = bottom_right_center.x - barycentric.x;
                let y = bottom_right_center.y - barycentric.y;

                Ok((
                    vec4(
                        bottom_left_index,
                        top_left_index,
                        bottom_right_index,
                        top_right_index,
                    ),
                    vec2(x, y),
                    [
                        bottom_left_ratio,
                        top_left_ratio,
                        bottom_right_ratio,
                        top_right_ratio,
                    ],
                ))
            }
            // only bottom left and bottom right - center above
            (
                Some((bottom_left_index, bottom_left_center, left_ratio)),
                None,
                Some((bottom_right_index, bottom_right_center, right_ratio)),
                None,
            ) => Self::selector_of_two_x(
                Some((bottom_left_index, bottom_left_center, left_ratio)),
                Some((bottom_right_index, bottom_right_center, right_ratio)),
                barycentric,
                "",
            ),
            // only top left and top right - center below
            (
                None,
                Some((top_left_index, top_left_center, left_ratio)),
                None,
                Some((top_right_index, top_right_center, right_ratio)),
            ) => Self::selector_of_two_x(
                Some((top_left_index, top_left_center, left_ratio)),
                Some((top_right_index, top_right_center, right_ratio)),
                barycentric,
                "",
            ),
            // only top right and bottom right - center left
            (
                None,
                None,
                Some((bottom_right_index, bottom_right_center, bottom_ratio)),
                Some((top_right_index, top_right_center, top_ratio)),
            ) => Self::selector_of_two_y(
                Some((top_right_index, top_right_center, top_ratio)),
                Some((bottom_right_index, bottom_right_center, bottom_ratio)),
                barycentric,
                "",
            ),
            // only top left and bottom left - center right
            (
                Some((bottom_left_index, bottom_left_center, bottom_ratio)),
                Some((top_left_index, top_left_center, top_ratio)),
                None,
                None,
            ) => Self::selector_of_two_y(
                Some((top_left_index, top_left_center, top_ratio)),
                Some((bottom_left_index, bottom_left_center, bottom_ratio)),
                barycentric,
                "",
            ),
            // only bottom right - left top corner
            (None, None, Some((index, _, ratio)), None) => Ok((
                vec4(index, -1, -1, -1),
                vec2(0.0, 0.0),
                Self::single_ratio(ratio),
            )),
            // only top right - left bottom corner
            (None, None, None, Some((index, _, ratio))) => Ok((
                vec4(index, -1, -1, -1),
                vec2(0.0, 0.0),
                Self::single_ratio(ratio),
            )),
            // only bottom left - right top corner
            (Some((index, _, ratio)), None, None, None) => Ok((
                vec4(index, -1, -1, -1),
                vec2(0.0, 0.0),
                Self::single_ratio(ratio),
            )),
            // only top left - right bottom corner
            (None, Some((index, _, ratio)), None, None) => Ok((
                vec4(index, -1, -1, -1),
                vec2(0.0, 0.0),
                Self::single_ratio(ratio),
            )),
            _ => create_error!("no fitting constellation found!"),
        }
    }

    #[inline]
    fn single_ratio(ratio: PlaneImageRatios) -> [PlaneImageRatios; 4] {
        [
            ratio,
            PlaneImageRatios::default(),
            PlaneImageRatios::default(),
            PlaneImageRatios::default(),
        ]
    }

    #[inline]
    fn two_ratios(
        first_ratio: PlaneImageRatios,
        second_ratio: PlaneImageRatios,
    ) -> [PlaneImageRatios; 4] {
        [
            first_ratio,
            second_ratio,
            PlaneImageRatios::default(),
            PlaneImageRatios::default(),
        ]
    }
}
