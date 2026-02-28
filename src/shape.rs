use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::f32::consts::PI;

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
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RawShape {
    start_pos: [f32; 4],
    end_pos: [f32; 4],
    color: [f32; 4],
    shape_type: u32,
    radius: f32,
    _padding: [f32; 2],
}

impl Shape {
    pub fn to_raw(&self) -> RawShape {
        match self {
            Shape::Sphere {
                origin,
                color,
                radius,
            } => RawShape {
                start_pos: [origin.x, origin.y, origin.z, 0.0],
                end_pos: [0.0, 0.0, 0.0, 0.0],
                color: [color.x, color.y, color.z, 0.0],
                shape_type: 0,
                radius: *radius,
                _padding: [0.0, 0.0],
            },
            Shape::Cylinder {
                start,
                end,
                color,
                radius,
            } => RawShape {
                start_pos: [start.x, start.y, start.z, 0.0],
                end_pos: [end.x, end.y, end.z, 0.0],
                color: [color.x, color.y, color.z, 0.0],
                shape_type: 1,
                radius: *radius,
                _padding: [0.0, 0.0],
            },
        }
    }
}

// Code was taken from here: https://www.songho.ca/opengl/gl_sphere.html
// Stacks go medially while sectors go laterally Creating a sphere shape from
// a bunch of sectors (subdivided into 2 triangles) arranged spherically.
pub fn generate_sphere_mesh(
    stack_count: usize,
    sector_count: usize,
    radius: f32,
) -> (Vec<Vec3>, Vec<Vec3>, Vec<usize>) {
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut indices: Vec<usize> = Vec::new();

    let sector_step = 2.0 * PI / (sector_count as f32);
    let stack_step = PI / (stack_count as f32);
    let length_inv = 1.0 / radius;

    // Generate the sphere vertices
    for i in 0..=stack_count {
        let stack_angle = PI / 2.0 - (i as f32) * stack_step;
        let xy = radius * stack_angle.cos();
        let z = radius * stack_angle.sin();

        for j in 0..=sector_count {
            let sector_angle = (j as f32) * sector_step;
            let v = Vec3::new(xy * sector_angle.cos(), xy * sector_angle.sin(), z);
            normals.push(Vec3::new(
                v.x * length_inv,
                v.y * length_inv,
                v.z * length_inv,
            ));
            vertices.push(v);
        }
    }

    // Generate the sphere indices
    for i in 0..stack_count {
        let mut k1 = i * (sector_count + 1);
        let mut k2 = k1 + sector_count + 1;

        for _ in 0..sector_count {
            if i != 0 {
                indices.push(k1);
                indices.push(k2);
                indices.push(k1 + 1);
            }

            if i != stack_count - 1 {
                indices.push(k1 + 1);
                indices.push(k2);
                indices.push(k2 + 1);
            }

            k1 += 1;
            k2 += 1;
        }
    }

    (vertices, normals, indices)
}

// Code was taken from here: https://www.songho.ca/opengl/gl_cylinder.html
// Same general algorithm as the sphere generation
// NOTE: the cylinder is uncapped because the ends of the cylinder will be covered up anyways
// in the scene
pub fn generate_cylinder_mesh(
    sector_count: usize,
    radius: f32,
    height: f32,
) -> (Vec<Vec3>, Vec<Vec3>, Vec<usize>) {
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut indices: Vec<usize> = Vec::new();

    let sector_step = 2.0 * PI / (sector_count as f32);

    // Generate the cylinder vertices
    for i in 0..2 {
        let h = -height / 2.0 + (i as f32) * height; // -h/2 to h/2

        for j in 0..=sector_count {
            let sector_angle = (j as f32) * sector_step;
            let ux = sector_angle.cos();
            let uy = sector_angle.sin();
            let uz = 0.0;

            vertices.push(Vec3::new(ux * radius, uy * radius, h));
            normals.push(Vec3::new(ux, uy, uz));
        }
    }

    // Generate the cylinder indices
    let mut k1 = 0;
    let mut k2 = sector_count + 1;

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

    (vertices, normals, indices)
}
