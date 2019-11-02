use context::prelude::*;

use cgmath::{vec3, Deg, Matrix3, Matrix4, Point3, Vector3};

use std::cell::Cell;
use std::sync::Arc;

const DIRECTION: Vector3<f32> = vec3(0.0, 1.0, 0.0);

pub struct ViewEmulator {
    context: Arc<Context>,

    position: Cell<Point3<f32>>,
    direction: Cell<Deg<f32>>,

    x_dir: Cell<i32>,
    y_dir: Cell<i32>,
    turn_dir: Cell<i32>,

    turn_speed: Deg<f32>,
    movement_speed: f32,

    last_time: Cell<f64>,

    // simulate vr transform for rendering without VR
    simulation_transform: Cell<VRTransformations>,
}

impl ViewEmulator {
    pub fn new(
        context: &Arc<Context>,
        turn_speed: impl Into<Deg<f32>>,
        movement_speed: f32,
    ) -> Self {
        let angle = Deg(0.0);

        let position = Point3::new(0.0, 0.0, 0.0);
        let direction = Matrix3::from_angle_z(angle) * DIRECTION;

        let simulation_transform = VRTransformations {
            proj: perspective(
                45.0,
                context.render_core().width() as f32 / context.render_core().height() as f32,
                0.01,
                100.0,
            ),
            view: Matrix4::look_at(position, position + direction, vec3(0.0, 0.0, 1.0)),
        };

        ViewEmulator {
            context: context.clone(),

            position: Cell::new(position),
            direction: Cell::new(angle),

            x_dir: Cell::new(0),
            y_dir: Cell::new(0),
            turn_dir: Cell::new(0),

            turn_speed: turn_speed.into(),
            movement_speed,

            last_time: Cell::new(context.time()),

            simulation_transform: Cell::new(simulation_transform),
        }
    }

    pub fn update(&self) -> VerboseResult<()> {
        let time_diff = self.context.time() - self.last_time.get();
        self.last_time.set(self.context.time());

        if self.turn_dir.get() != 0 || self.x_dir.get() != 0 || self.y_dir.get() != 0 {
            if self.turn_dir.get() < 0 {
                self.direction
                    .set(self.direction.get() + self.turn_speed * time_diff as f32);
            } else if self.turn_dir.get() > 0 {
                self.direction
                    .set(self.direction.get() - self.turn_speed * time_diff as f32);
            }

            let dir = self.direction();

            if self.x_dir.get() < 0 {
                let left_dir = vec3(-dir.y, dir.x, dir.z) * self.movement_speed;

                self.position
                    .set(self.position.get() + left_dir * time_diff as f32);
            } else if self.x_dir.get() > 0 {
                let right_dir = vec3(dir.y, -dir.x, dir.z) * self.movement_speed;

                self.position
                    .set(self.position.get() + right_dir * time_diff as f32);
            }

            if self.y_dir.get() < 0 {
                let new_dir = vec3(dir.x, dir.y, dir.z) * self.movement_speed;

                self.position
                    .set(self.position.get() - new_dir * time_diff as f32);
            } else if self.y_dir.get() > 0 {
                let new_dir = vec3(dir.x, dir.y, dir.z) * self.movement_speed;

                self.position
                    .set(self.position.get() + new_dir * time_diff as f32);
            }

            let mut transform = self.simulation_transform.get();
            let dir = self.direction();
            transform.view = Matrix4::look_at(
                self.position.get(),
                self.position.get() + vec3(dir.x, dir.y, dir.z),
                vec3(0.0, 0.0, 1.0),
            );

            self.simulation_transform.set(transform);
        }

        Ok(())
    }

    pub fn on_key_down(&self, key: Keycode) {
        match key {
            Keycode::A => self.x_dir.set(self.x_dir.get() - 1),
            Keycode::D => self.x_dir.set(self.x_dir.get() + 1),
            Keycode::W => self.y_dir.set(self.y_dir.get() + 1),
            Keycode::S => self.y_dir.set(self.y_dir.get() - 1),
            Keycode::Q => self.turn_dir.set(self.turn_dir.get() - 1),
            Keycode::E => self.turn_dir.set(self.turn_dir.get() + 1),
            _ => (),
        }
    }

    pub fn on_key_up(&self, key: Keycode) {
        match key {
            Keycode::A => self.x_dir.set(self.x_dir.get() + 1),
            Keycode::D => self.x_dir.set(self.x_dir.get() - 1),
            Keycode::W => self.y_dir.set(self.y_dir.get() - 1),
            Keycode::S => self.y_dir.set(self.y_dir.get() + 1),
            Keycode::Q => self.turn_dir.set(self.turn_dir.get() + 1),
            Keycode::E => self.turn_dir.set(self.turn_dir.get() - 1),
            _ => (),
        }
    }

    pub fn simulation_transform(&self) -> VRTransformations {
        self.simulation_transform.get()
    }

    #[inline]
    fn direction(&self) -> Vector3<f32> {
        Matrix3::from_angle_z(self.direction.get()) * DIRECTION
    }
}
