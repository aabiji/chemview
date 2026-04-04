use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Quat, Vec2, Vec3};
use std::collections::HashMap;
use std::f32;
use std::f32::consts::PI;
use std::hash::Hash;
use std::hash::Hasher;

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
    Curve {
        points: Vec<Vec3>,
        min_point: Vec3,
        max_point: Vec3,
    },
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
            Shape::Curve {
                min_point,
                max_point,
                ..
            } => (min_point, max_point),
        }
    }

    pub fn translate(&mut self, offset: Vec3) {
        match self {
            Shape::Sphere { origin, .. } => *origin -= offset,
            Shape::Cylinder { start, end, .. } => {
                *start -= offset;
                *end -= offset;
            }
            Shape::Curve { points, .. } => {
                points.iter_mut().for_each(|p| *p -= offset);
            }
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

#[repr(C)]
#[derive(Clone, Default, Pod, Zeroable, Copy)]
pub struct Vertex {
    pub position: [f32; 4],
    pub normal: [f32; 4],
}

impl Vertex {
    fn from(pos: Vec3, normal: Vec3) -> Vertex {
        Vertex {
            position: [pos[0], pos[1], pos[2], 1.0],
            normal: [normal[0], normal[1], normal[2], 0.0],
        }
    }
}

fn vec3_key(v: &Vec3) -> (u32, u32, u32) {
    (v.x.to_bits(), v.y.to_bits(), v.z.to_bits())
}

pub fn generate_sphere_mesh(steps: usize) -> (Vec<Vertex>, Vec<u32>) {
    const PHI: f32 = 1.618033988749894;

    // Start off with the 20 faces of a unit icosahedron
    #[rustfmt::skip]
    let mut vertices: Vec<Vec3> = vec![
        Vec3::new(0.0, 1.0, PHI), Vec3::new(0.0, 1.0, -PHI),
        Vec3::new(0.0, -1.0, PHI), Vec3::new(0.0, -1.0, -PHI),
        Vec3::new(1.0, PHI, 0.0), Vec3::new(1.0, -PHI, 0.0),
        Vec3::new(-1.0, PHI, 0.0), Vec3::new(-1.0, -PHI, 0.0),
        Vec3::new(PHI, 0.0, 1.0), Vec3::new(PHI, 0.0, -1.0),
        Vec3::new(-PHI, 0.0, 1.0), Vec3::new(-PHI, 0.0, -1.0),
    ];

    #[rustfmt::skip]
    let mut faces: Vec<(usize, usize, usize)> = vec![
        (0, 2, 8), (0, 8, 4), (0, 4, 6), (0, 6, 10),
        (0, 10, 2), (3, 9, 1), (3, 1, 11), (3, 11, 7),
        (3, 7, 5), (3, 5, 9), (2, 5, 8), (8, 5, 9),
        (8, 9, 4), (4, 9, 1), (4, 1, 6), (6, 1, 11),
        (6, 11, 10), (10, 11, 7), (10, 7, 2), (2, 7, 5),
    ];

    // Fragment the original faces into smaller sub faces over multiple steps to approximate a sphere
    for _ in 0..steps {
        let mut new_vertices: Vec<Vec3> = Vec::new();
        let mut new_faces: Vec<(usize, usize, usize)> = Vec::new();

        // Map a point to its corresponding index to avoid duplicates
        let mut vertex_map: HashMap<(u32, u32, u32), usize> = HashMap::new();

        // Fragment each face into four faces that are slightly projected outwards
        for face in &faces {
            let (v1, v2, v3) = (vertices[face.0], vertices[face.1], vertices[face.2]);
            let (v4, v5, v6) = (
                ((v1 + v2) * 0.5).normalize(),
                ((v2 + v3) * 0.5).normalize(),
                ((v3 + v1) * 0.5).normalize(),
            );

            let idx = [v1, v2, v3, v4, v5, v6].map(|v| {
                let k = vec3_key(&v);
                if !vertex_map.contains_key(&k) {
                    vertex_map.insert(k, new_vertices.len());
                    new_vertices.push(v);
                }
                vertex_map[&k]
            });

            new_faces.push((idx[0], idx[3], idx[5]));
            new_faces.push((idx[3], idx[1], idx[4]));
            new_faces.push((idx[5], idx[4], idx[2]));
            new_faces.push((idx[3], idx[4], idx[5]));
        }

        vertices = new_vertices;
        faces = new_faces
    }

    (
        vertices
            .iter()
            .map(|v| Vertex::from(v.normalize(), v.normalize()))
            .collect(),
        faces
            .iter()
            .flat_map(|&(a, b, c)| [a as u32, b as u32, c as u32])
            .collect(),
    )
}

pub fn generate_uncapped_cylinder_mesh(
    sector_count: usize,
    radius: f32,
    height: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Two rings of vertices: one at z = 0 (i = 0) and one at z = height (i = 1)
    // Each ring has sector_count+1 vertices (first and last overlap in XY to close the loop)
    let sector_step = 2.0 * PI / (sector_count as f32);
    for i in 0..2 {
        let h = (i as f32) * height; // 0 to h

        for j in 0..=sector_count {
            let sector_angle = (j as f32) * sector_step;
            // Unit circle direction for this sector — normals point straight outward (no z component)
            let ux = sector_angle.cos();
            let uy = sector_angle.sin();
            let uz = 0.0;

            let v = Vec3::new(ux * radius, uy * radius, h);
            let n = Vec3::new(ux, uy, uz);
            vertices.push(Vertex::from(v, n));
        }
    }

    // Connect the two rings with quads (two triangles each)
    // k1 walks the bottom ring, k2 walks the top ring in lockstep
    let mut k1 = 0;
    let mut k2 = (sector_count + 1) as u32;

    for _ in 0..sector_count {
        indices.push(k1);
        indices.push(k1 + 1);
        indices.push(k2);

        indices.push(k2);
        indices.push(k1 + 1);
        indices.push(k2 + 1);

        k1 += 1;
        k2 += 1;
    }

    (vertices, indices)
}

// Following the matrix multiply from
// https://andrewhungblog.wordpress.com/2017/03/03/catmull-rom-splines-in-plain-english/
// p are the control points, t is the step and A is the tension parameter
fn catmull_rom_spline(p: &[Vec3; 4], t: f32, A: f32) -> Vec3 {
    let w0 = 0.5 * (A * -A + A * A * 2.0 * A - A * A * A * A);
    let w1 = 0.5 * (2.0 + A * A * (A - 6.0) + A * A * A * (4.0 - A));
    let w2 = 0.5 * (A * A + A * A * -2.0 * (A - 3.0) + A * A * A * (A - 4.0));
    let w3 = 0.5 * (A * A * -A + A * A * A * A);
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
pub fn generate_curve_mesh(points: Vec<Vec3>) -> (Vec<Vec3>, Vec<u32>) {
    let num_steps = 3;
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
    let mut vertices: Vec<Vec3> = Vec::new();

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

            // Place each cross section point into 3D
            for c in &cross_section {
                vertices.push(p + c.x * N + c.y * B);
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
            let a = j * n + k;
            let b = j * n + (k + 1) % n; // wrap around
            let c = (j + 1) * n + k;
            let d = (j + 1) * n + (k + 1) % n;
            indices.extend(&[a as u32, c as u32, b as u32]);
            indices.extend(&[b as u32, c as u32, d as u32]);
        }
    }

    (vertices, indices)
}
