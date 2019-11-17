use context::prelude::*;

use cgmath::{vec3, Matrix4, Rad, Vector3, Zero};

use std::collections::HashMap;

const CAMERA_X_COUNT: &str = "num_cams_x";
const CAMERA_Y_COUNT: &str = "num_cams_y";
const BASELINE: &str = "baseline_mm";
const FOCUS_DISTANCE: &str = "focus_distance_m";
const CAMERA_CENTER_X: &str = "center_cam_x_m";
const CAMERA_CENTER_Y: &str = "center_cam_y_m";
const CAMERA_CENTER_Z: &str = "center_cam_z_m";
const CAMERA_RX: &str = "center_cam_rx_rad";
const CAMERA_RY: &str = "center_cam_ry_rad";
const CAMERA_RZ: &str = "center_cam_rz_rad";
const OFFSET: &str = "offset";

#[derive(Debug, PartialEq)]
pub struct Extrinsic {
    pub horizontal_camera_count: u32,
    pub vertical_camera_count: u32,

    /// in millimeters
    baseline: f32,

    /// in meters
    pub focus_distance: f32,

    /// in meters
    pub camera_center: Vector3<f32>,

    /// in radians
    pub camera_rotation: Vector3<Rad<f32>>,

    pub offset: f32,
}

impl Extrinsic {
    pub fn load(data: &HashMap<String, Value>) -> VerboseResult<Self> {
        let mut config = Self::default();

        config.horizontal_camera_count = data
            .get(CAMERA_X_COUNT)
            .ok_or("camera x count not present")?
            .to_value()?;

        config.vertical_camera_count = data
            .get(CAMERA_Y_COUNT)
            .ok_or("camera y count not present")?
            .to_value()?;

        config.baseline = data
            .get(BASELINE)
            .ok_or("baseline not present")?
            .to_value()?;

        config.focus_distance = data
            .get(FOCUS_DISTANCE)
            .ok_or("focus distance not present")?
            .to_value()?;

        config.offset = data.get(OFFSET).ok_or("offset not present")?.to_value()?;

        let cam_x = data
            .get(CAMERA_CENTER_X)
            .ok_or("camera center x not present")?
            .to_value()?;
        let cam_y = data
            .get(CAMERA_CENTER_Y)
            .ok_or("camera center y not present")?
            .to_value()?;
        let cam_z = data
            .get(CAMERA_CENTER_Z)
            .ok_or("camera center z not present")?
            .to_value()?;

        let cam_rx = data
            .get(CAMERA_RX)
            .ok_or("camera rx not present")?
            .to_value()?;
        let cam_ry = data
            .get(CAMERA_RY)
            .ok_or("camera ry not present")?
            .to_value()?;
        let cam_rz = data
            .get(CAMERA_RZ)
            .ok_or("camera rz not present")?
            .to_value()?;

        config.camera_center = vec3(cam_x, cam_y, cam_z);
        config.camera_rotation = vec3(Rad(cam_rx), Rad(cam_ry), Rad(cam_rz));

        Ok(config)
    }

    pub fn camera_rotation_matrix(&self) -> Matrix4<f32> {
        // https://www.mauriciopoppe.com/notes/computer-graphics/transformation-matrices/rotation/euler-angles/
        Matrix4::from_angle_z(self.camera_rotation.z)
            * Matrix4::from_angle_y(self.camera_rotation.y)
            * Matrix4::from_angle_x(self.camera_rotation.x)
    }

    // baseline in meters
    pub fn baseline(&self) -> f32 {
        self.baseline * 0.001
    }
}

impl Default for Extrinsic {
    fn default() -> Self {
        Extrinsic {
            horizontal_camera_count: 0,
            vertical_camera_count: 0,
            baseline: 0.0,
            focus_distance: 0.0,
            camera_center: Vector3::zero(),
            camera_rotation: vec3(Rad(0.0), Rad(0.0), Rad(0.0)),
            offset: 0.0,
        }
    }
}
