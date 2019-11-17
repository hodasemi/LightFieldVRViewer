use context::prelude::*;

use cgmath::Vector3;

use crate::config::Config;

#[derive(Debug)]
pub struct LightFieldFrustum {
    position: (usize, usize),

    top_left: Vector3<f32>,
    bottom_left: Vector3<f32>,
    top_right: Vector3<f32>,
    bottom_right: Vector3<f32>,
}

impl LightFieldFrustum {
    pub fn create(
        center: Vector3<f32>,
        direction: Vector3<f32>,
        up: Vector3<f32>,
        right: Vector3<f32>,
        config: &Config,
    ) -> Vec<LightFieldFrustum> {
        let sensor_center = center - direction * config.intrinsics.focal_length();

        unimplemented!()
    }

    pub fn position(&self) -> (usize, usize) {
        self.position
    }
}
