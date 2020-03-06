use context::prelude::*;

use std::collections::HashMap;

#[doc(hidden)]
const FOCAL_LENGTH: &str = "focal_length_mm";

#[doc(hidden)]
const IMAGE_WIDTH: &str = "image_resolution_x_px";

#[doc(hidden)]
const IMAGE_HEIGHT: &str = "image_resolution_y_px";

#[doc(hidden)]
const SENSOR_SIZE: &str = "sensor_size_mm";

#[doc(hidden)]
const FSTOP: &str = "fstop";

/// Rust equivalent to the intrinsic part
#[derive(Debug, PartialEq)]
pub struct Intrinsic {
    /// in millimeters
    focal_length: f32,

    /// in pixels
    pub image_width: u32,

    /// in pixels
    pub image_height: u32,

    /// in millimeters
    sensor_size: f32,

    fstop: f32,
}

impl Intrinsic {
    #[doc(hidden)]
    pub fn load(data: &HashMap<String, Value>) -> VerboseResult<Self> {
        Ok(Intrinsic {
            focal_length: data
                .get(FOCAL_LENGTH)
                .ok_or("focal length not present")?
                .to_value()?,

            image_width: data
                .get(IMAGE_WIDTH)
                .ok_or("image width not present")?
                .to_value()?,

            image_height: data
                .get(IMAGE_HEIGHT)
                .ok_or("image height not present")?
                .to_value()?,

            sensor_size: data
                .get(SENSOR_SIZE)
                .ok_or("sensor_size not present")?
                .to_value()?,

            fstop: data.get(FSTOP).ok_or("fstop not present")?.to_value()?,
        })
    }

    /// Focal length in meter
    pub fn focal_length(&self) -> f32 {
        self.focal_length * 0.001
    }

    /// Sensor size in meter
    pub fn sensor_size(&self) -> f32 {
        self.sensor_size * 0.001
    }

    /// fstop in meter
    #[allow(unused)]
    pub fn fstop(&self) -> f32 {
        self.fstop * 0.001
    }
}
