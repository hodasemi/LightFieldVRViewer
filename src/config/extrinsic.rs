use context::prelude::*;

use crate::error::{LightFieldError, Result};

use cgmath::{vec3, Rad, Vector3, Zero};

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
    pub baseline: f32,

    /// in meters
    pub focus_distance: f32,

    /// in meters
    pub camera_center: Vector3<f32>,

    /// in radians
    pub camera_rotation: Vector3<Rad<f32>>,

    pub offset: f32,
}

impl Extrinsic {
    pub fn load(data: &HashMap<String, Value>) -> Result<Self> {
        let horizontal_camera_count = data
            .get(CAMERA_X_COUNT)
            .ok_or(LightFieldError::config_loader("camera x count not present"))?;
        let vertical_camera_count = data
            .get(CAMERA_Y_COUNT)
            .ok_or(LightFieldError::config_loader("camera y count not present"))?;
        let baseline = data
            .get(BASELINE)
            .ok_or(LightFieldError::config_loader("baseline not present"))?;
        let focus_distance = data
            .get(FOCUS_DISTANCE)
            .ok_or(LightFieldError::config_loader("focus distance not present"))?;
        let cam_x = data
            .get(CAMERA_CENTER_X)
            .ok_or(LightFieldError::config_loader(
                "camera center x not present",
            ))?
            .apply_value()?;
        let cam_y = data
            .get(CAMERA_CENTER_Y)
            .ok_or(LightFieldError::config_loader(
                "camera center y not present",
            ))?
            .apply_value()?;
        let cam_z = data
            .get(CAMERA_CENTER_Z)
            .ok_or(LightFieldError::config_loader(
                "camera center z not present",
            ))?
            .apply_value()?;
        let cam_rx = data
            .get(CAMERA_RX)
            .ok_or(LightFieldError::config_loader("camera rx not present"))?
            .apply_value()?;
        let cam_ry = data
            .get(CAMERA_RY)
            .ok_or(LightFieldError::config_loader("camera ry not present"))?
            .apply_value()?;
        let cam_rz = data
            .get(CAMERA_RZ)
            .ok_or(LightFieldError::config_loader("camera rz not present"))?
            .apply_value()?;
        let offset = data
            .get(OFFSET)
            .ok_or(LightFieldError::config_loader("offset not present"))?;

        let mut config = Self::default();

        horizontal_camera_count.set_value(&mut config.horizontal_camera_count)?;
        vertical_camera_count.set_value(&mut config.vertical_camera_count)?;
        baseline.set_value(&mut config.baseline)?;
        focus_distance.set_value(&mut config.focus_distance)?;
        offset.set_value(&mut config.offset)?;

        config.camera_center = vec3(cam_x, cam_y, cam_z);
        config.camera_rotation = vec3(Rad(cam_rx), Rad(cam_ry), Rad(cam_rz));

        Ok(config)
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
