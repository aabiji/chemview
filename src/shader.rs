use std::fs::File;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use wgpu::BindGroupLayout;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferUsages, Device,
    ShaderStages, util::BufferInitDescriptor, util::DeviceExt,
};

pub fn load_shader_source(path: &PathBuf) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub struct ShaderVar {
    pub is_f32: bool,
    pub is_storage: bool,
    pub num_elements: usize,
    pub label: &'static str,
}

pub const STORAGE_BUFFER_SIZE: usize = 10 * 1024 * 1024;

const SHADER_VARS: [ShaderVar; 7] = [
    ShaderVar {
        is_f32: true,
        is_storage: false,
        num_elements: 16,
        label: "Projection matrix",
    },
    ShaderVar {
        is_f32: true,
        is_storage: false,
        num_elements: 16,
        label: "View matrix",
    },
    ShaderVar {
        is_f32: true,
        is_storage: false,
        num_elements: 4,
        label: "Camera position",
    },
    ShaderVar {
        is_f32: true,
        is_storage: false,
        num_elements: 2,
        label: "Resolution",
    },
    ShaderVar {
        is_f32: true,
        is_storage: false,
        num_elements: 16,
        label: "Object rotation",
    },
    ShaderVar {
        is_f32: true,
        is_storage: true,
        num_elements: STORAGE_BUFFER_SIZE,
        label: "Shapes",
    },
    ShaderVar {
        is_f32: true,
        is_storage: true,
        num_elements: 4,
        label: "Num shapes",
    },
];

pub fn create_shader_vars(device: &Device) -> (BindGroupLayout, BindGroup, Vec<Buffer>) {
    let layout_entries: Vec<BindGroupLayoutEntry> = SHADER_VARS
        .iter()
        .enumerate()
        .map(|(index, v)| BindGroupLayoutEntry {
            binding: index as u32,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: if v.is_storage {
                    BufferBindingType::Storage { read_only: true }
                } else {
                    BufferBindingType::Uniform
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect();

    let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Main bind group layout"),
        entries: &layout_entries,
    });

    let buffers: Vec<Buffer> = SHADER_VARS
        .iter()
        .map(|v| {
            let contents = if v.is_f32 {
                bytemuck::cast_slice(&vec![0.0f32; v.num_elements]).to_vec()
            } else {
                bytemuck::cast_slice(&vec![0u32; v.num_elements]).to_vec()
            };

            device.create_buffer_init(&BufferInitDescriptor {
                label: Some(v.label),
                contents: &contents,
                usage: if v.is_storage {
                    BufferUsages::STORAGE | BufferUsages::COPY_DST
                } else {
                    BufferUsages::UNIFORM | BufferUsages::COPY_DST
                },
            })
        })
        .collect();

    let entries: Vec<BindGroupEntry> = SHADER_VARS
        .iter()
        .enumerate()
        .map(|(index, _)| BindGroupEntry {
            binding: index as u32,
            resource: buffers[index].as_entire_binding(),
        })
        .collect();

    let group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Main bind group"),
        layout: &layout,
        entries: &entries,
    });

    (layout, group, buffers)
}
