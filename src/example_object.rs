use context::prelude::*;

use cgmath::{vec2, vec3, Vector2, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExampleVertex {
    pub position: Vector3<f32>,
    pub uv: Vector2<f32>,
}

impl ExampleVertex {
    pub fn new(x: f32, y: f32, z: f32, u: f32, v: f32) -> Self {
        ExampleVertex {
            position: vec3(x, y, z),
            uv: vec2(u, v),
        }
    }
}

// pub struct ExampleObject {}
