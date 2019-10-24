use context::prelude::*;

use crate::error::{LightFieldError, Result};

use std::collections::HashMap;

const FOCAL_LENGTH: &str = "focal_length_mm";
const IMAGE_WIDTH: &str = "image_resolution_x_px";
const IMAGE_HEIGHT: &str = "image_resolution_y_px";
const SENSOR_SIZE: &str = "sensor_size_mm";
const FSTOP: &str = "fstop";

#[derive(Debug, PartialEq)]
pub struct Intrinsic {
    /// in millimeters
    pub focal_length: f32,

    /// in pixels
    pub image_width: u32,

    /// in pixels
    pub image_height: u32,

    /// in millimeters
    pub sensor_size: f32,

    pub fstop: f32,
}

impl Intrinsic {
    pub fn load(data: &HashMap<String, Value>) -> Result<Self> {
        Ok(Intrinsic {
            focal_length: data
                .get(FOCAL_LENGTH)
                .ok_or(LightFieldError::config_loader("focal length not present"))?
                .apply_value()?,

            image_width: data
                .get(IMAGE_WIDTH)
                .ok_or(LightFieldError::config_loader("image width not present"))?
                .apply_value()?,

            image_height: data
                .get(IMAGE_HEIGHT)
                .ok_or(LightFieldError::config_loader("image height not present"))?
                .apply_value()?,

            sensor_size: data
                .get(SENSOR_SIZE)
                .ok_or(LightFieldError::config_loader("sensor_size not present"))?
                .apply_value()?,

            fstop: data
                .get(FSTOP)
                .ok_or(LightFieldError::config_loader("fstop not present"))?
                .apply_value()?,
        })
    }
}
