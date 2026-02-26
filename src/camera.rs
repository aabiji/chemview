use glam::{Vec2, Vec3};
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
        // -z will be made to go into the screen
        Self {
            pitch: 0.0,
            yaw: -90.0,
            field_of_view: 45.0,
            front: Vec3::new(0.0, 0.0, -1.0),
            position: Vec3::new(0.0, 0.0, -3.0),
        }
    }

    pub fn position(&self) -> [f32; 4] {
        [self.position.x, self.position.y, self.position.z, 0.0]
    }

    pub fn padded_basis(&self) -> [f32; 12] {
        let right = self.front.cross(Vec3::Y).normalize();
        let up = right.cross(self.front).normalize();
        [
            right.x,
            right.y,
            right.z,
            0.0,
            up.x,
            up.y,
            up.z,
            0.0,
            self.front.x,
            self.front.y,
            self.front.z,
            0.0, // right
        ]
    }

    fn translate(&mut self, m: Action, speed: f32) {
        let up = Vec3::Y;
        let right = self.front.cross(up).normalize();
        match m {
            Action::Up => self.position -= up * speed,
            Action::Down => self.position += up * speed,
            Action::Left => self.position -= right * speed,
            Action::Right => self.position += right * speed,
            Action::Forward => self.position -= self.front * speed,
            Action::Backward => self.position += self.front * speed,
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
            sensitivity: 0.5,
            speed: 0.6,
        }
    }

    pub fn camera_state(&self) -> ([f32; 4], [f32; 12]) {
        (self.camera.position(), self.camera.padded_basis())
    }

    pub fn zoom(&mut self, inwards: bool) {
        self.camera.zoom(inwards);
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_down = pressed;
    }

    pub fn update_mouse_delta(&mut self, x: f32, y: f32) {
        self.mouse_delta = Vec2::new(self.prev_mouse.x - x, self.prev_mouse.y - y);
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

    pub fn update_camera(&mut self) {
        for action in &self.actions {
            self.camera.translate(*action, self.speed);
        }
        if self.mouse_down {
            self.camera.rotate(self.mouse_delta.x, self.mouse_delta.y);
        }
        self.mouse_delta = Vec2::new(0.0, 0.0);
    }
}
