use glam::{Vec2, Vec3};

pub enum Translate {
    Up,
    Down,
    Left,
    Right,
    Forward,
    Backward,
}

pub struct Camera {
    position: Vec3,
    pitch: f32,
    yaw: f32,
    field_of_view: f32,
    speed: f32,
    sensitivity: f32,
    prev_mouse_pos: Vec2,
    front: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            pitch: 0.0,
            yaw: 90.0,
            field_of_view: 45.0,
            speed: 1.0,
            sensitivity: 0.05,
            prev_mouse_pos: Vec2::new(0.0, 0.0),
            front: Vec3::new(0.0, 0.0, 1.0),
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

    pub fn translate(&mut self, m: Translate) {
        let up = Vec3::Y;
        let right = self.front.cross(up).normalize();
        match m {
            Translate::Up => self.position += up * self.speed,
            Translate::Down => self.position -= up * self.speed,
            Translate::Left => self.position -= right * self.speed,
            Translate::Right => self.position += right * self.speed,
            Translate::Forward => self.position += self.front * self.speed,
            Translate::Backward => self.position -= self.front * self.speed,
        }
    }

    pub fn rotate(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool) {
        let offset = Vec2::new(
            (mouse_x - self.prev_mouse_pos.x) * self.sensitivity,
            (self.prev_mouse_pos.y - mouse_y) * self.sensitivity,
        );
        self.prev_mouse_pos = Vec2::new(mouse_x, mouse_y);
        if !mouse_down {
            return;
        }

        self.yaw += offset.x;
        self.pitch = (self.pitch + offset.y).clamp(-89.9, 89.9);

        let front = Vec3::new(
            self.yaw.to_radians().cos() * self.pitch.to_radians().cos(),
            self.pitch.to_radians().sin(),
            self.yaw.to_radians().sin() * self.pitch.to_radians().cos(),
        );
        self.front = front.normalize();
    }

    pub fn zoom(&mut self, inwards: bool) {
        let offset = if inwards { -1.0 } else { 1.0 };
        self.field_of_view = (self.field_of_view + offset).clamp(1.0, 45.0);
    }
}
