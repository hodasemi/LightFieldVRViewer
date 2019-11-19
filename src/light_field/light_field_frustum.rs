// #![allow(unused)]

use context::prelude::*;

use cgmath::{InnerSpace, Vector3};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct Line {
    pub center: Vector3<f32>,
    pub direction: Vector3<f32>,
}

impl Line {
    fn create(center: Vector3<f32>, helper: Vector3<f32>) -> Line {
        Line {
            center,
            direction: (center - helper).normalize(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LightFieldFrustum {
    position: (usize, usize),

    pub left_top: Line,
    pub left_bottom: Line,
    pub right_top: Line,
    pub right_bottom: Line,

    main_direction: Vector3<f32>,
}

impl LightFieldFrustum {
    /// `direction`, `up`, `right` need to be normalized
    pub fn create_frustums(
        center: Vector3<f32>,
        direction: Vector3<f32>,
        up: Vector3<f32>,
        right: Vector3<f32>,
        config: &Config,
    ) -> Vec<LightFieldFrustum> {
        let mut frustums = Vec::new();
        let baseline = config.extrinsics.baseline();

        let width = config.extrinsics.horizontal_camera_count as usize;
        let height = config.extrinsics.vertical_camera_count as usize;

        let total_top_left = center - (((width - 1) / 2) as f32 * baseline * right)
            + (((height - 1) / 2) as f32 * baseline * up);

        for x in 0..width {
            for y in 0..height {
                let current_center =
                    total_top_left + (x as f32 * baseline * right) - (y as f32 * baseline * up);

                frustums.push(Self::as_f_stop(
                    x,
                    y,
                    current_center,
                    direction,
                    up,
                    right,
                    config,
                ))
            }
        }

        frustums
    }

    // fn as_aperture_size(
    //     x: usize,
    //     y: usize,
    //     camera_center: Vector3<f32>,
    //     direction: Vector3<f32>,
    //     up: Vector3<f32>,
    //     right: Vector3<f32>,
    //     config: &Config,
    // ) -> Self {
    //     let sensor_center = camera_center - direction * config.intrinsics.focal_length();

    //     let (sensor_left_top, sensor_left_bottom, sensor_right_top, sensor_right_bottom) =
    //         Self::calculate_corners(sensor_center, up, right, config.intrinsics.sensor_size());

    //     let (aperture_left_top, aperture_left_bottom, aperture_right_top, aperture_right_bottom) =
    //         Self::calculate_corners(camera_center, up, right, config.intrinsics.fstop());

    //     let left_top = Line::create(aperture_left_top, sensor_left_top);
    //     let left_bottom = Line::create(aperture_left_bottom, sensor_left_bottom);
    //     let right_top = Line::create(aperture_right_top, sensor_right_top);
    //     let right_bottom = Line::create(aperture_right_bottom, sensor_right_bottom);

    //     LightFieldFrustum {
    //         position: (x, y),

    //         left_top,
    //         left_bottom,
    //         right_top,
    //         right_bottom,
    //     }
    // }

    fn as_f_stop(
        x: usize,
        y: usize,
        camera_center: Vector3<f32>,
        direction: Vector3<f32>,
        up: Vector3<f32>,
        right: Vector3<f32>,
        config: &Config,
    ) -> Self {
        let sensor_center = camera_center - direction * config.intrinsics.focal_length();

        let (sensor_left_top, sensor_left_bottom, sensor_right_top, sensor_right_bottom) =
            Self::calculate_corners(sensor_center, up, right, config.intrinsics.sensor_size());

        let left_top = Line::create(camera_center, sensor_right_bottom);
        let left_bottom = Line::create(camera_center, sensor_right_top);
        let right_top = Line::create(camera_center, sensor_left_bottom);
        let right_bottom = Line::create(camera_center, sensor_left_top);

        LightFieldFrustum {
            position: (x, y),

            left_top,
            left_bottom,
            right_top,
            right_bottom,

            main_direction: direction,
        }
    }

    /// Returns the 4 corner points
    /// (`left top`, `left bottom`, `right top`, `right bottom`)
    pub fn get_corners_at_depth(
        &self,
        depth: f32,
    ) -> (Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>) {
        (
            Self::calculate_target_point(
                &self.left_top,
                Self::calculate_length_for_direction(
                    self.main_direction,
                    self.left_top.direction,
                    depth,
                ),
            ),
            Self::calculate_target_point(
                &self.left_bottom,
                Self::calculate_length_for_direction(
                    self.main_direction,
                    self.left_bottom.direction,
                    depth,
                ),
            ),
            Self::calculate_target_point(
                &self.right_top,
                Self::calculate_length_for_direction(
                    self.main_direction,
                    self.right_top.direction,
                    depth,
                ),
            ),
            Self::calculate_target_point(
                &self.right_bottom,
                Self::calculate_length_for_direction(
                    self.main_direction,
                    self.right_bottom.direction,
                    depth,
                ),
            ),
        )
    }

    #[inline]
    fn calculate_length_for_direction(
        base_direction: Vector3<f32>,
        direction: Vector3<f32>,
        base_length: f32,
    ) -> f32 {
        unimplemented!()
    }

    #[inline]
    fn calculate_target_point(line: &Line, length: f32) -> Vector3<f32> {
        line.center + line.direction * length
    }

    #[inline]
    fn calculate_corners(
        center: Vector3<f32>,
        up: Vector3<f32>,
        right: Vector3<f32>,
        distance: f32,
    ) -> (Vector3<f32>, Vector3<f32>, Vector3<f32>, Vector3<f32>) {
        let horizontal = right * distance;
        let vertical = up * distance;

        let left_top = center - horizontal + vertical;
        let left_bottom = center - horizontal - vertical;
        let right_top = center + horizontal + vertical;
        let right_bottom = center + horizontal - vertical;

        (left_top, left_bottom, right_top, right_bottom)
    }

    pub fn position(&self) -> (usize, usize) {
        self.position
    }
}
