use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Quat, Vec2, Vec3};
use std::f32::consts::PI;
use std::hash::Hash;
use std::hash::Hasher;

#[repr(C)]
#[derive(Clone, Default, Pod, Zeroable, Copy)]
pub struct Vertex {
    pub position: [f32; 4],
    pub normal: [f32; 4],
    pub color: [f32; 4],
}

impl Vertex {
    fn from(pos: Vec3, normal: Vec3, color: Vec3) -> Vertex {
        Vertex {
            position: [pos[0], pos[1], pos[2], 1.0],
            normal: [normal[0], normal[1], normal[2], 0.0],
            color: [color[0], color[1], color[2], 0.0],
        }
    }
}

#[derive(Clone)]
pub enum Shape {
    Sphere {
        origin: Vec3,
        color: Vec3,
        radius: f32,
    },
    Cylinder {
        start: Vec3,
        end: Vec3,
        color: Vec3,
        radius: f32,
    },
}

#[repr(C)]
#[derive(Clone, Default, Pod, Zeroable, Copy)]
pub struct RawShape {
    start_pos: [f32; 4],
    end_pos: [f32; 4],
    color: [f32; 4],
    radius: f32,
    shape_type: u32,
    _padding: u32,
}

impl Shape {
    pub fn bounds(&self) -> (Vec3, Vec3) {
        match *self {
            Shape::Sphere { origin, radius, .. } => (
                origin - Vec3::splat(radius), // leftmost, bottommost, innermost
                origin + Vec3::splat(radius), // rightmost, topmost, outermost
            ),
            Shape::Cylinder {
                start, end, radius, ..
            } => (
                start.min(end) - Vec3::splat(radius), // leftmost
                start.max(end) + Vec3::splat(radius), // rightmost
            ),
        }
    }

    pub fn translate(&self, offset: Vec3) -> Shape {
        match *self {
            Shape::Sphere {
                origin,
                color,
                radius,
            } => Shape::Sphere {
                origin: origin - offset,
                color,
                radius,
            },
            Shape::Cylinder {
                start,
                end,
                color,
                radius,
            } => Shape::Cylinder {
                start: start - offset,
                end: end - offset,
                color,
                radius,
            },
        }
    }

    pub fn raw(&self) -> RawShape {
        match *self {
            Shape::Sphere {
                origin,
                color,
                radius,
            } => RawShape {
                start_pos: [origin.x, origin.y, origin.z, 0.0],
                end_pos: [0.0, 0.0, 0.0, 0.0],
                color: [color.x, color.y, color.z, 1.0],
                radius,
                shape_type: 0,
                _padding: 0,
            },
            Shape::Cylinder {
                start,
                end,
                color,
                radius,
            } => RawShape {
                start_pos: [start.x, start.y, start.z, 0.0],
                end_pos: [end.x, end.y, end.z, 0.0],
                color: [color.x, color.y, color.z, 1.0],
                radius,
                shape_type: 1,
                _padding: 0,
            },
        }
    }
}

impl Eq for Shape {}

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&Shape::Sphere { origin: a, .. }, &Shape::Sphere { origin: b, .. }) => a == b,
            _ => todo!("Implement PartialEq for Cylinder!"),
        }
    }
}

impl Hash for Shape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            Shape::Sphere { origin, .. } => {
                origin.x.to_bits().hash(state);
                origin.y.to_bits().hash(state);
                origin.z.to_bits().hash(state);
            }
            _ => todo!("Implement Hash for Cylinder!"),
        }
    }
}

// Following the matrix multiply from
// https://andrewhungblog.wordpress.com/2017/03/03/catmull-rom-splines-in-plain-english/
// p are the control points, t is the step and A is the tension parameter
fn catmull_rom_spline(p: &[Vec3; 4], t: f32, A: f32) -> Vec3 {
    let w0 = 0.5 * (-A * t + 2.0 * A * t * t - A * t * t * t);
    let w1 = 0.5 * (2.0 + 0.0 + (A - 6.0) * t * t + (4.0 - A) * t * t * t);
    let w2 = 0.5 * (A * t - 2.0 * (A - 3.0) * t * t - A * t * t * t);
    let w3 = 0.5 * (-A * t * t + A * t * t * t);
    w0 * p[0] + w1 * p[1] + w2 * p[2] + w3 * p[3]
}

// Sample n points around an ellipse with semi-axes w and h
fn ellipse_cross_section(w: f32, h: f32, n: u32) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let angle = 2.0 * PI * ((i as f32) / (n as f32));
            Vec2::new(w * angle.cos(), h * angle.sin())
        })
        .collect()
}

// Following this paper: https://pmc.ncbi.nlm.nih.gov/articles/PMC2672931/
pub fn generate_curve_mesh(
    points: &[Vec3],
    index_offset: usize,
    num_steps: usize,
    color: Vec3,
) -> (Vec<Vertex>, Vec<u32>) {
    let cross_section = ellipse_cross_section(1.5, 0.25, 8);

    // Extrapolate the first and last points so that they can be included in the curve
    // To extrapolate, just mirror the point in the opposite direction
    let first = points[0] + (points[0] - points[1]);
    let last = points[points.len() - 1] + (points[points.len() - 1] - points[points.len() - 2]);
    let mut padded = vec![first];
    padded.extend(points.iter().copied());
    padded.push(last);

    // Compute the xyz axis for each local frame
    // A "local frame" is just a way to map the xy coordinates of a
    // point on a cross section, to a 3d point in world space
    let local_frames: Vec<Mat3> = (1..padded.len() - 1)
        .map(|i| {
            // N (normal) = x, B (binormal) = y, T (tangent) = z
            // V is just a helper vector on the same plane as T
            let T = (padded[i + 1] - padded[i - 1]).normalize();
            let V = (padded[i] - padded[i - 1]).normalize();
            let B = V.cross(T).normalize();
            let N = B.cross(T).normalize();
            Mat3::from_cols(N, B, T)
        })
        .collect();

    // Compute the curve vertices
    let mut vertices: Vec<Vertex> = Vec::new();

    for i in 1..padded.len() - 2 {
        let q1 = Quat::from_mat3(&local_frames[i - 1]);
        let q2 = Quat::from_mat3(&local_frames[i]);

        for j in 0..num_steps {
            // Interpolated point
            let t = (j as f32) * (1.0 / num_steps as f32);
            let p = catmull_rom_spline(
                &[padded[i - 1], padded[i], padded[i + 1], padded[i + 2]],
                t,
                0.25,
            );

            // Interpolated frame
            let q = q1.slerp(q2, t);
            let N = q.mul_vec3(Vec3::X);
            let B = q.mul_vec3(Vec3::Y);

            // Map each cross section point into 3D
            for c in &cross_section {
                let placed = p + c.x * N + c.y * B;
                let normal = (placed - p).normalize();
                vertices.push(Vertex::from(placed, normal, color));
            }
        }
    }

    let mut indices: Vec<u32> = Vec::new();

    let total_steps = (padded.len() - 3) * num_steps;
    let n = cross_section.len();

    // Step along the curve
    for j in 0..total_steps - 1 {
        // Step along the cross section
        for k in 0..n {
            // Make a quad between adjacent pair of vertices around the ring and split it
            let a = index_offset + j * n + k;
            let b = index_offset + j * n + (k + 1) % n; // wrap around
            let c = index_offset + (j + 1) * n + k;
            let d = index_offset + (j + 1) * n + (k + 1) % n;
            indices.extend(&[a as u32, c as u32, b as u32]);
            indices.extend(&[b as u32, c as u32, d as u32]);
        }
    }

    (vertices, indices)
}
