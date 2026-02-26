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

pub struct ShaderVar {
    pub is_f32: bool,
    pub is_storage: bool,
    pub num_bytes: usize,
    pub label: String,
}

pub fn load_shader_source(path: &PathBuf) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn setup_shader_vars(
    device: &Device,
    vars: &Vec<ShaderVar>,
) -> (Vec<Buffer>, BindGroupLayout, BindGroup) {
    let buffers: Vec<Buffer> = vars
        .iter()
        .map(|v| {
            let contents = if v.is_f32 {
                bytemuck::cast_slice(&vec![0.0f32; v.num_bytes]).to_vec()
            } else {
                bytemuck::cast_slice(&vec![0u32; v.num_bytes]).to_vec()
            };

            device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&v.label),
                contents: &contents,
                usage: if v.is_storage {
                    BufferUsages::STORAGE | BufferUsages::COPY_DST
                } else {
                    BufferUsages::UNIFORM | BufferUsages::COPY_DST
                },
            })
        })
        .collect();

    let layout_entries: Vec<BindGroupLayoutEntry> = vars
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

    let entries: Vec<BindGroupEntry> = vars
        .iter()
        .enumerate()
        .map(|(index, _)| BindGroupEntry {
            binding: index as u32,
            resource: buffers[index].as_entire_binding(),
        })
        .collect();

    let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Main bind group layout"),
        entries: &layout_entries,
    });

    let group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Vertex shader bind group"),
        layout: &layout,
        entries: &entries,
    });

    (buffers, layout, group)
}
