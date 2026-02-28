use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::PI;
use std::ops::Range;

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
pub struct InstanceData {
    model_matrix: [[f32; 4]; 4],
    color: [f32; 4],
}

pub fn to_raw(shape: &Shape) -> InstanceData {
    match *shape {
        Shape::Sphere {
            origin,
            color,
            radius,
        } => {
            let model = Mat4::from_translation(origin) * Mat4::from_scale(Vec3::splat(radius));
            InstanceData {
                model_matrix: model.to_cols_array_2d(),
                color: [color.x, color.y, color.z, 1.0],
            }
        }
        Shape::Cylinder {
            start,
            end,
            color,
            radius,
        } => {
            // Create a transformation matrix that orientes the cylinder from start to end
            let direction = end - start;
            let length = direction.length();
            let rotation = Quat::from_rotation_arc(Vec3::Z, direction.normalize());
            let model = Mat4::from_translation(start)
                * Mat4::from_quat(rotation)
                * Mat4::from_scale(Vec3::new(radius, radius, length));
            InstanceData {
                model_matrix: model.to_cols_array_2d(),
                color: [color.x, color.y, color.z, 1.0],
            }
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
            position: [pos[0], pos[1], pos[2], 0.0],
            normal: [normal[0], normal[1], normal[2], 0.0],
        }
    }
}

// Code was taken from here: https://www.songho.ca/opengl/gl_sphere.html
// Stacks go medially while sectors go laterally Creating a sphere shape from
// a bunch of sectors (subdivided into 2 triangles) arranged spherically.
fn generate_sphere_mesh(
    stack_count: usize,
    sector_count: usize,
    radius: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

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
            let n = Vec3::new(v.x * length_inv, v.y * length_inv, v.z * length_inv);
            vertices.push(Vertex::from(v, n));
        }
    }

    // Generate the sphere indices
    for i in 0..stack_count {
        let mut k1 = (i as u32) * (sector_count + 1) as u32;
        let mut k2 = (k1 + (sector_count as u32) + 1) as u32;

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

    (vertices, indices)
}

// Code was taken from here: https://www.songho.ca/opengl/gl_cylinder.html
// Same general algorithm as the sphere generation
// NOTE: the cylinder is uncapped because the ends of the cylinder will be covered up anyways
// in the scene
fn generate_cylinder_mesh(
    sector_count: usize,
    radius: f32,
    height: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let sector_step = 2.0 * PI / (sector_count as f32);

    // Generate the cylinder vertices
    for i in 0..2 {
        let h = -height / 2.0 + (i as f32) * height; // -h/2 to h/2

        for j in 0..=sector_count {
            let sector_angle = (j as f32) * sector_step;
            let ux = sector_angle.cos();
            let uy = sector_angle.sin();
            let uz = 0.0;

            let v = Vec3::new(ux * radius, uy * radius, h);
            let n = Vec3::new(ux, uy, uz);
            vertices.push(Vertex::from(v, n));
        }
    }

    // Generate the cylinder indices
    let mut k1 = 0 as u32;
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

// Create a vertex buffer and an index buffer that combines the vertices and
// indices for the sphere and cylinders. Seperate them by index ranegs
pub fn create_mesh_buffers(
    stack_count: usize,
    sector_count: usize,
    radius: f32,
    height: f32,
) -> (Vec<Vertex>, Vec<u32>, Range<u32>, Range<u32>) {
    let (v1, i1) = generate_sphere_mesh(stack_count, sector_count, radius);
    let (v2, i2) = generate_cylinder_mesh(sector_count, radius, height);

    // Offset the indices for the cylinder vertices
    let sphere_vertex_count = v1.len() as u32;
    let i2_offset: Vec<u32> = i2.iter().map(|i| i + sphere_vertex_count).collect();

    let sphere_index_range = 0..i1.len() as u32;
    let cylinder_index_range = i1.len() as u32..(i1.len() + i2.len()) as u32;

    (
        [v1.as_slice(), v2.as_slice()].concat(),
        [i1.as_slice(), i2_offset.as_slice()].concat(),
        sphere_index_range,
        cylinder_index_range,
    )
}
