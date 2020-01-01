use context::prelude::*;

use cgmath::{vec3, Deg, Matrix3, Matrix4, Point3, Vector3};

use std::sync::Arc;

use crate::light_field_viewer::{DEFAULT_FORWARD, UP};

pub struct ViewEmulator {
    context: Arc<Context>,

    position: Point3<f32>,
    direction: Deg<f32>,

    slow_mode: bool,

    x_dir: i32,
    y_dir: i32,
    z_dir: i32,

    turn_dir: i32,

    turn_speed: Deg<f32>,
    movement_speed: f32,

    last_time: f64,

    // simulate vr transform for rendering without VR
    simulation_transform: VRTransformations,
}

impl ViewEmulator {
    pub fn new(
        context: &Arc<Context>,
        turn_speed: impl Into<Deg<f32>>,
        movement_speed: f32,
    ) -> Self {
        let angle = Deg(0.0);

        let position = Point3::new(0.0, 0.0, 0.0);
        let direction = Matrix3::from_axis_angle(UP, angle) * DEFAULT_FORWARD;

        let simulation_transform = VRTransformations {
            proj: perspective(
                45.0,
                context.render_core().width() as f32 / context.render_core().height() as f32,
                0.01,
                1000.0,
            ),
            view: Matrix4::look_at(position, position + direction, UP),
        };

        ViewEmulator {
            context: context.clone(),

            position: position,
            direction: angle,

            slow_mode: false,

            x_dir: 0,
            y_dir: 0,
            z_dir: 0,
            turn_dir: 0,

            turn_speed: turn_speed.into(),
            movement_speed,

            last_time: context.time(),

            simulation_transform: simulation_transform,
        }
    }

    pub fn update(&mut self) -> VerboseResult<()> {
        let time_diff = self.context.time() - self.last_time;
        self.last_time = self.context.time();

        // check for any direction change
        if self.turn_dir != 0 || self.x_dir != 0 || self.z_dir != 0 || self.y_dir != 0 {
            // check for rotation
            if self.turn_dir < 0 {
                self.direction = self.direction + self.turn_speed() * time_diff as f32;
            } else if self.turn_dir > 0 {
                self.direction = self.direction - self.turn_speed() * time_diff as f32;
            }

            let dir = self.direction();

            // check for left/right movement
            if self.x_dir < 0 {
                let left_dir = vec3(dir.z, dir.y, -dir.x) * self.movement_speed();

                self.position = self.position + left_dir * time_diff as f32;
            } else if self.x_dir > 0 {
                let right_dir = vec3(-dir.z, dir.y, dir.x) * self.movement_speed();

                self.position = self.position + right_dir * time_diff as f32;
            }

            // check for forward/backward movement
            if self.z_dir < 0 {
                let new_dir = vec3(dir.x, dir.y, dir.z) * self.movement_speed();

                self.position = self.position - new_dir * time_diff as f32;
            } else if self.z_dir > 0 {
                let new_dir = vec3(dir.x, dir.y, dir.z) * self.movement_speed();

                self.position = self.position + new_dir * time_diff as f32;
            }

            // check for up/down lift
            if self.y_dir < 0 {
                let new_dir = UP * self.movement_speed();

                self.position = self.position - new_dir * time_diff as f32;
            } else if self.y_dir > 0 {
                let new_dir = UP * self.movement_speed();

                self.position = self.position + new_dir * time_diff as f32;
            }

            let mut transform = self.simulation_transform;
            let dir = self.direction();
            transform.view =
                Matrix4::look_at(self.position, self.position + vec3(dir.x, dir.y, dir.z), UP);

            self.simulation_transform = transform;
        }

        Ok(())
    }

    pub fn on_key_down(&mut self, key: Keycode) {
        match key {
            Keycode::A => self.x_dir -= 1,
            Keycode::D => self.x_dir += 1,
            Keycode::W => self.z_dir += 1,
            Keycode::S => self.z_dir -= 1,
            Keycode::Q => self.turn_dir -= 1,
            Keycode::E => self.turn_dir += 1,
            Keycode::Space => self.y_dir += 1,
            Keycode::LCtrl => self.y_dir -= 1,
            Keycode::LShift => self.slow_mode = true,
            _ => (),
        }
    }

    pub fn on_key_up(&mut self, key: Keycode) {
        match key {
            Keycode::A => self.x_dir += 1,
            Keycode::D => self.x_dir -= 1,
            Keycode::W => self.z_dir -= 1,
            Keycode::S => self.z_dir += 1,
            Keycode::Q => self.turn_dir += 1,
            Keycode::E => self.turn_dir -= 1,
            Keycode::Space => self.y_dir -= 1,
            Keycode::LCtrl => self.y_dir += 1,
            Keycode::LShift => self.slow_mode = false,
            _ => (),
        }
    }

    pub fn on_resize(&mut self) {
        self.simulation_transform.proj = perspective(
            45.0,
            self.context.render_core().width() as f32 / self.context.render_core().height() as f32,
            0.01,
            1000.0,
        );
    }

    pub fn simulation_transform(&self) -> VRTransformations {
        self.simulation_transform
    }

    #[inline]
    fn movement_speed(&self) -> f32 {
        if self.slow_mode {
            self.movement_speed * 0.25
        } else {
            self.movement_speed
        }
    }

    #[inline]
    fn turn_speed(&self) -> Deg<f32> {
        if self.slow_mode {
            self.turn_speed * 0.25
        } else {
            self.turn_speed
        }
    }

    #[inline]
    fn direction(&self) -> Vector3<f32> {
        Matrix3::from_axis_angle(UP, self.direction) * DEFAULT_FORWARD
    }
}
