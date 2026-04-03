use bytemuck::offset_of;
use glam::{Mat4, Quat, Vec3};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferAddress, BufferUsages, CommandEncoder,
    DepthBiasState, DepthStencilState, Device, DeviceDescriptor, Extent3d, FragmentState, LoadOp,
    MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState, Queue,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor,
    StencilState, Surface, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
    VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::shader;
use crate::shape::{self, Shape, Vertex};
use crate::ui::{DebugUI, UIState};
use crate::{
    camera::CameraController,
    shader::{GLOBAL_SHADER_VARS, INSTANCE_SHADER_VARS},
};

struct ShapeInstance {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    bind_group: BindGroup,
    buffers: Vec<Buffer>,
    num_indices: u32,

    // For each instance
    model_matrices: Vec<[[f32; 4]; 4]>,
    colors: Vec<[f32; 4]>,
}

impl ShapeInstance {
    fn new(
        device: &Device,
        layout: &BindGroupLayout,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    ) -> Self {
        let (buffers, bind_group) = shader::create_buffers(device, layout, &INSTANCE_SHADER_VARS);

        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Vertex buffer (sphere)"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX,
            }),

            index_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Index buffer (sphere)"),
                contents: bytemuck::cast_slice(&indices),
                usage: BufferUsages::INDEX,
            }),
            buffers,
            bind_group,
            num_indices: indices.len() as u32,
            model_matrices: Vec::new(),
            colors: Vec::new(),
        }
    }

    fn ranges(&self) -> (Range<u32>, Range<u32>) {
        (0..self.num_indices, 0..self.model_matrices.len() as u32)
    }
}

// The maximum size in bytes of a storage buffer will be 10 MB
const DEPTH_TEXTURE_FORMAT: TextureFormat = TextureFormat::Depth24Plus;
const MSAA_SAMPLE_COUNT: u32 = 4;

fn create_texture(
    device: &Device,
    format: TextureFormat,
    width: u32,
    height: u32,
    label: &str,
) -> TextureView {
    // The texture will be used for antialiasing
    let texture = device.create_texture(&TextureDescriptor {
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        label: Some(label),
        view_formats: &[],
    });
    texture.create_view(&TextureViewDescriptor::default())
}

pub struct Renderer {
    pub window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    render_pipeline: RenderPipeline,

    bind_group: BindGroup,
    buffers: Vec<Buffer>,
    msaa_texture: TextureView,
    depth_texture: TextureView,
    instances: HashMap<usize, ShapeInstance>,

    pub ui: DebugUI,
    pub controller: CameraController,
    current_time: SystemTime,

    // `surface` should be the last to get dropped
    surface: Surface<'static>,
    surface_format: TextureFormat,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .unwrap();
        let window_size = window.inner_size();
        let surface = instance.create_surface(window.clone()).unwrap();
        let surface_format = surface.get_capabilities(&adapter).formats[0];

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
        let shader_source = shader::load_shader_source(&path).unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let msaa_texture = create_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
            "MSAA Texture",
        );

        let depth_texture = create_texture(
            &device,
            DEPTH_TEXTURE_FORMAT,
            window_size.width,
            window_size.height,
            "Depth texture",
        );
        let global_bind_group_layout =
            shader::create_bind_group_layout(&device, &GLOBAL_SHADER_VARS);

        let instance_bind_group_layout =
            shader::create_bind_group_layout(&device, &INSTANCE_SHADER_VARS);

        let (global_buffers, global_bind_group) =
            shader::create_buffers(&device, &global_bind_group_layout, &GLOBAL_SHADER_VARS);

        let mut instances: HashMap<usize, ShapeInstance> = HashMap::new();
        let (vertices, indices) = shape::generate_sphere_mesh(32, 32, 1.0);
        instances.insert(
            0,
            ShapeInstance::new(&device, &instance_bind_group_layout, vertices, indices),
        );
        let (vertices, indices) = shape::generate_cylinder_mesh(32, 1.0, 1.0);
        instances.insert(
            1,
            ShapeInstance::new(&device, &instance_bind_group_layout, vertices, indices),
        );

        let vertex_buffers = [VertexBufferLayout {
            array_stride: size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: offset_of!(Vertex, position) as u64,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: offset_of!(Vertex, normal) as u64,
                    shader_location: 1,
                },
            ],
        }];

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render pipeline layout"),
                bind_group_layouts: &[&global_bind_group_layout, &instance_bind_group_layout],
                push_constant_ranges: &[],
            })),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vertex_shader"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fragment_shader"),
                targets: &[Some(surface_format.add_srgb_suffix().into())],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_TEXTURE_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: MSAA_SAMPLE_COUNT,
                ..MultisampleState::default()
            },
            cache: None,
            multiview: None,
        });

        let ui = DebugUI::new(&device, &window, surface_format);

        let state = Renderer {
            window,
            window_size,

            device,
            queue,
            render_pipeline,

            bind_group: global_bind_group,
            buffers: global_buffers,
            msaa_texture,
            depth_texture,
            instances,

            ui,
            controller: CameraController::default(),
            current_time: SystemTime::now(),

            surface,
            surface_format,
        };
        state.configure_surface();
        state
    }

    fn configure_surface(&self) {
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            desired_maximum_frame_latency: 2,
            width: self.window_size.width,
            height: self.window_size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &config);
    }

    pub fn get_window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.window_size = size;
        self.msaa_texture = create_texture(
            &self.device,
            self.surface_format.add_srgb_suffix(),
            size.width,
            size.height,
            "MSAA Texture",
        );
        self.depth_texture = create_texture(
            &self.device,
            DEPTH_TEXTURE_FORMAT,
            size.width,
            size.height,
            "Depth buffer",
        );
        self.configure_surface();
    }

    fn add_shape(&mut self, shape: &Shape) {
        let id = match *shape {
            Shape::Sphere { .. } => 0,
            Shape::Cylinder { .. } => 1,
        };
        let batch = self.instances.get_mut(&id).unwrap();

        match *shape {
            Shape::Sphere {
                origin,
                color,
                radius,
            } => {
                let model = Mat4::from_translation(origin) * Mat4::from_scale(Vec3::splat(radius));
                batch.model_matrices.push(model.to_cols_array_2d());
                batch.colors.push([color.x, color.y, color.z, 1.0]);
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
                batch.model_matrices.push(model.to_cols_array_2d());
                batch.colors.push([color.x, color.y, color.z, 1.0]);
            }
        }
    }

    pub fn set_mesh_data(&mut self, data: &(Vec<Shape>, Vec3, Vec3)) {
        let target_pos = Vec3::new(0.0, 0.0, 0.0);
        let (bounding_min, bounding_max) = (data.1, data.2);
        let size = bounding_max - bounding_min;

        let offset = (bounding_min + size / 2.0) - target_pos;
        for shape in &data.0 {
            let mut copy = shape.clone();
            copy.translate(offset);
            self.add_shape(&copy);
        }

        for instance in self.instances.values() {
            self.queue.write_buffer(
                &instance.buffers[0],
                0,
                bytemuck::cast_slice(&instance.model_matrices),
            );

            self.queue.write_buffer(
                &instance.buffers[1],
                0,
                bytemuck::cast_slice(&instance.colors),
            );
        }
    }

    fn update_shader_vars(&mut self) {
        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` function. Make sure they match!
        let ratio = (self.window_size.width as f32) / (self.window_size.height as f32);
        let (position, projection, view, object_rotation) = self.controller.camera_state(ratio);

        self.queue
            .write_buffer(&self.buffers[0], 0, bytemuck::cast_slice(&projection));

        self.queue
            .write_buffer(&self.buffers[1], 0, bytemuck::cast_slice(&view));

        self.queue
            .write_buffer(&self.buffers[2], 0, bytemuck::cast_slice(&object_rotation));

        self.queue
            .write_buffer(&self.buffers[3], 0, bytemuck::cast_slice(&position));
    }

    fn render_shapes(
        &mut self,
        encoder: &mut CommandEncoder,
        surface_texture_view: &TextureView,
        delta_time: f32,
    ) {
        self.controller.update_camera(1.0 / delta_time);
        self.update_shader_vars();

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Main render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.msaa_texture,
                    resolve_target: Some(surface_texture_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);

            for instance in self.instances.values() {
                pass.set_bind_group(1, &instance.bind_group, &[]);
                pass.set_index_buffer(instance.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_vertex_buffer(0, instance.vertex_buffer.slice(..));

                let (index_range, instance_range) = instance.ranges();
                pass.draw_indexed(index_range, 0, instance_range);
            }
        }
    }

    pub fn render(&mut self, ui_state: &mut UIState) -> f32 {
        let now = SystemTime::now();
        let delta_time = now.duration_since(self.current_time).unwrap().as_millis() as f32;
        self.current_time = now;

        let surface_texture = self.surface.get_current_texture().unwrap();
        let surface_texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
            format: Some(self.surface_format.add_srgb_suffix()),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.render_shapes(&mut encoder, &surface_texture_view, delta_time);
        self.ui.render(
            &self.device,
            &self.window,
            &self.queue,
            &mut encoder,
            &surface_texture_view,
            ui_state,
        );

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();

        // fps: 1.0 / delta_time_in_secs
        1.0 / (delta_time / 1000.0)
    }
}
