use glam::{Mat4, Vec2, Vec3};
use std::collections::HashSet;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Forward,
    Backward,
}

struct Camera {
    position: Vec3,
    pitch: f32,
    yaw: f32,
    field_of_view: f32,
    front: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        let pitch = 0.0f32;
        let yaw = -90.0f32;
        let front = Vec3::new(
            yaw.to_radians().cos() * pitch.to_radians().cos(),
            pitch.to_radians().sin(),
            yaw.to_radians().sin() * pitch.to_radians().cos(),
        )
        .normalize();
        Self {
            pitch,
            yaw,
            field_of_view: 45.0,
            front,
            position: Vec3::new(0.0, 0.0, 3.0),
        }
    }

    pub fn position(&self) -> [f32; 4] {
        [self.position.x, self.position.y, self.position.z, 0.0]
    }

    pub fn projection(&self, aspect_ratio: f32) -> [[f32; 4]; 4] {
        let fov = self.field_of_view.to_radians();
        Mat4::perspective_rh(fov, aspect_ratio, 0.1, 100.0).to_cols_array_2d()
    }

    pub fn view(&self) -> [[f32; 4]; 4] {
        Mat4::look_at_rh(self.position, self.position + self.front, Vec3::Y).to_cols_array_2d()
    }

    fn translate(&mut self, m: Action, speed: f32) {
        let up = Vec3::Y;
        let right = self.front.cross(up).normalize();
        let front = self.front.normalize();
        match m {
            Action::Up => self.position += up * speed,
            Action::Down => self.position -= up * speed,
            Action::Left => self.position -= right * speed,
            Action::Right => self.position += right * speed,
            Action::Forward => self.position += front * speed,
            Action::Backward => self.position -= front * speed,
        }
    }

    fn rotate(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x;
        self.pitch = (self.pitch + delta_y).clamp(-89.9, 89.9);

        let front = Vec3::new(
            self.yaw.to_radians().cos() * self.pitch.to_radians().cos(),
            self.pitch.to_radians().sin(),
            self.yaw.to_radians().sin() * self.pitch.to_radians().cos(),
        );
        self.front = front.normalize();
    }

    fn zoom(&mut self, inwards: bool) {
        let offset = if inwards { -1.0 } else { 1.0 };
        self.field_of_view = (self.field_of_view + offset).clamp(1.0, 45.0);
    }
}

pub struct CameraController {
    camera: Camera,
    actions: HashSet<Action>,
    mouse_down: bool,
    prev_mouse: Vec2,
    mouse_delta: Vec2,
    sensitivity: f32,
    speed: f32,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            camera: Camera::new(),
            actions: HashSet::new(),
            mouse_down: false,
            prev_mouse: Vec2::new(0.0, 0.0),
            mouse_delta: Vec2::new(0.0, 0.0),
            sensitivity: 2.5,
            speed: 2.5,
        }
    }

    pub fn camera_state(&self, aspect_ratio: f32) -> ([f32; 4], [[f32; 4]; 4], [[f32; 4]; 4]) {
        (
            self.camera.position(),
            self.camera.projection(aspect_ratio),
            self.camera.view(),
        )
    }

    pub fn zoom(&mut self, inwards: bool) {
        self.camera.zoom(inwards);
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_down = pressed;
    }

    pub fn update_mouse_delta(&mut self, x: f32, y: f32) {
        self.mouse_delta = Vec2::new(x - self.prev_mouse.x, self.prev_mouse.y - y);
        self.mouse_delta *= self.sensitivity;
        self.prev_mouse = Vec2::new(x, y);
    }

    pub fn set_action(&mut self, action: Action, pressed: bool) {
        if pressed {
            self.actions.insert(action);
        } else {
            self.actions.remove(&action);
        }
    }

    pub fn update_camera(&mut self, delta_time: f32) {
        for action in &self.actions {
            self.camera.translate(*action, self.speed * delta_time);
        }
        if self.mouse_down {
            self.camera.rotate(
                self.mouse_delta.x * delta_time,
                self.mouse_delta.y * delta_time,
            );
        }
        self.mouse_delta = Vec2::new(0.0, 0.0);
    }
}
