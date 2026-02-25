/*
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::{borrow::Cow, io::Read};
use wgpu::BindGroupLayout;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferUsages, Device,
    DeviceDescriptor, Extent3d, FragmentState, MultisampleState, PipelineLayoutDescriptor,
    PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderStages, Surface,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexState, util::BufferInitDescriptor, util::DeviceExt,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

mod camera;
use crate::camera::{Camera, Translate};

mod refactor;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SDFData {
    // Position is in world space
    position: [f32; 3],
    _padding: f32, // makes `position` 4 byte aligned
    color: [f32; 3],
    radius: f32, // conventiently padding `color`
}

impl SDFData {
    pub fn from(position: Vec3, color: Vec3, radius: f32) -> Self {
        SDFData {
            position: [position.x, position.y, position.z],
            _padding: 0.0,
            color: [color.x, color.y, color.z],
            radius,
        }
    }
}

// Pad f32 or vec2 to the 16 byte aligned wgsl expects
fn pad_vec2<T: Default>(a: T, b: T) -> [T; 4] {
    [a, b, T::default(), T::default()]
}

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    render_pipeline: RenderPipeline,

    msaa_texture: TextureView,
    sdf_data_buffer: Buffer,
    sdf_data_count: Buffer,
    view_uniform_buffer: Buffer,
    resolution_buffer: Buffer,
    camera_pos_buffer: Buffer,

    camera: Camera,
    mouse_down: bool,
    pressed_keys: HashSet<String>,

    // `surface` should be the last to get dropped
    surface: Surface<'static>,
    surface_format: TextureFormat,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
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

        let shader_source = State::load_shader_source().unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let msaa_texture = State::create_msaa_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
        );

        let camera = Camera::new();
        let mouse_down = false;
        let pressed_keys: HashSet<String> = HashSet::new();

        let view_uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Transformation matrix uniform buffer"),
            contents: bytemuck::cast_slice(&camera.padded_basis()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // These is initially filled with placeholder data
        let sdf_data_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("SDF data storage buffer"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, 0.0, 0.0]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let sdf_data_count = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("SDF data count uniform value"),
            contents: bytemuck::cast_slice(&pad_vec2::<u32>(0, 0)),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let resolution_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Window size uniform buffer"),
            contents: bytemuck::cast_slice(&pad_vec2::<f32>(
                window_size.width as f32,
                window_size.height as f32,
            )),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let camera_pos_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Camera position uniform buffer"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Main bind group layout"),
            entries: &[
                // View matrix
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // SDF Data
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // SDF data count
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Window resolution
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Camera position
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Vertex shader bind group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: view_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: sdf_data_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: sdf_data_count.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: resolution_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: camera_pos_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vertex_shader"),
                buffers: &[], // No vertex buffer, since a fullscreen quad is drawn in the vertex shader
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fragment_shader"),
                targets: &[Some(surface_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 4,
                ..MultisampleState::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let mut state = State {
            window,
            window_size,
            device,
            queue,
            bind_group,
            bind_group_layout,
            render_pipeline,
            view_uniform_buffer,
            sdf_data_buffer,
            sdf_data_count,
            resolution_buffer,
            camera_pos_buffer,
            msaa_texture,
            camera,
            mouse_down,
            pressed_keys,
            surface,
            surface_format,
        };
        state.configure_surface();
        state.update_sdf_data(vec![
            SDFData::from(Vec3::new(3.0, 1.0, 1.0), Vec3::new(0.0, 1.0, 0.0), 0.9),
            SDFData::from(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
            SDFData::from(Vec3::new(0.0, 3.0, 0.0), Vec3::new(1.0, 1.0, 0.0), 0.23),
            SDFData::from(Vec3::new(1.0, 0.0, 2.0), Vec3::new(0.0, 0.0, 1.0), 0.5),
        ]);
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

    fn load_shader_source() -> Result<String, io::Error> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn create_msaa_texture(
        device: &Device,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> TextureView {
        let texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            label: Some("MSAA Texture"),
            view_formats: &[],
        });
        texture.create_view(&TextureViewDescriptor::default())
    }

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_down = pressed;
    }

    fn set_key_press(&mut self, key: String, pressed: bool) {
        if pressed {
            self.pressed_keys.insert(key);
        } else {
            self.pressed_keys.remove(&key);
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.window_size = size;
        self.msaa_texture = State::create_msaa_texture(
            &self.device,
            self.surface_format.add_srgb_suffix(),
            self.window_size.width,
            self.window_size.height,
        );
        self.update_camera_transform();
        self.configure_surface();
    }

    fn update_sdf_data(&mut self, data: Vec<SDFData>) {
        self.sdf_data_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("SDF data storage buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        self.sdf_data_count = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("SDF data count uniform value"),
            contents: bytemuck::cast_slice(&pad_vec2::<u32>(data.len() as u32, 0)),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        self.bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Vertex shader bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.view_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.sdf_data_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.sdf_data_count.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: self.resolution_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: self.camera_pos_buffer.as_entire_binding(),
                },
            ],
        });
    }

    fn update_camera_transform(&mut self) {
        self.queue.write_buffer(
            &self.view_uniform_buffer,
            0,
            bytemuck::cast_slice(&self.camera.padded_basis()),
        );
        self.queue.write_buffer(
            &self.resolution_buffer,
            0,
            bytemuck::cast_slice(&pad_vec2::<f32>(
                self.window_size.width as f32,
                self.window_size.height as f32,
            )),
        );
        self.queue.write_buffer(
            &self.camera_pos_buffer,
            0,
            bytemuck::cast_slice(&self.camera.position()),
        );
    }

    fn update_keys(&mut self) {
        if self.pressed_keys.contains("w") {
            self.camera.translate(Translate::Forward);
        }
        if self.pressed_keys.contains("a") {
            self.camera.translate(Translate::Left);
        }
        if self.pressed_keys.contains("s") {
            self.camera.translate(Translate::Backward);
        }
        if self.pressed_keys.contains("d") {
            self.camera.translate(Translate::Right);
        }
        if self.pressed_keys.contains("U") {
            self.camera.translate(Translate::Down);
        }
        if self.pressed_keys.contains("D") {
            self.camera.translate(Translate::Up);
        }
    }

    fn render(&mut self) {
        self.update_keys();
        self.update_camera_transform();

        let surface_texture = self.surface.get_current_texture().unwrap();
        let surface_texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
            format: Some(self.surface_format.add_srgb_suffix()),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Main render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.msaa_texture,
                    resolve_target: Some(&surface_texture_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1); // A ullscreen quad is being drawn in the vertex shader
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_resizable(true)
                        .with_title("Chemview"),
                )
                .unwrap(),
        );

        let state = pollster::block_on(State::new(window.clone()));

        self.state = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::RedrawRequested => {
                state.render();
                state.get_window().request_redraw();
            }

            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => state.resize(size), // Resize will request a redraw

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: s,
                        ..
                    },
                ..
            } => {
                let pressed = s == ElementState::Pressed;

                match logical_key {
                    Key::Character(c) => {
                        if pressed {
                            state.set_key_press(c.to_string(), true);
                        } else {
                            state.set_key_press(c.to_string(), false);
                        }
                    }

                    Key::Named(k) => match k {
                        NamedKey::ArrowDown => state.set_key_press(String::from("D"), pressed),
                        NamedKey::ArrowUp => state.set_key_press(String::from("U"), pressed),
                        _ => {}
                    },
                    _ => {}
                }

                state.get_window().request_redraw();
            }

            WindowEvent::MouseInput {
                state: ms, button, ..
            } => {
                state.set_mouse_pressed(button == MouseButton::Left && ms == ElementState::Pressed)
            }

            WindowEvent::CursorMoved { position, .. } => {
                state
                    .camera
                    .rotate(position.x as f32, position.y as f32, state.mouse_down);
                state.get_window().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.camera.zoom(delta_y < 0.0);
                state.get_window().request_redraw();
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
*/
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::{borrow::Cow, io::Read};
use wgpu::BindGroupLayout;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferUsages, Device,
    DeviceDescriptor, Extent3d, FragmentState, MultisampleState, PipelineLayoutDescriptor,
    PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderStages, Surface,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexState, util::BufferInitDescriptor, util::DeviceExt,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

mod camera;
use crate::camera::{Action, CameraController};

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
    start_pos: [f32; 3],
    end_pos: [f32; 3],
    color: [f32; 3],
    shape_type: u32,
    radius: f32,
    _padding: [f32; 5],
}

impl Shape {
    fn to_raw(&self) -> RawShape {
        match self {
            Shape::Sphere {
                origin,
                color,
                radius,
            } => RawShape {
                start_pos: [origin.x, origin.y, origin.z],
                end_pos: [0.0, 0.0, 0.0],
                color: [color.x, color.y, color.z],
                shape_type: 0,
                radius: *radius,
                _padding: [0.0, 0.0, 0.0, 0.0, 0.0],
            },
            Shape::Cylinder {
                start,
                end,
                color,
                radius,
            } => RawShape {
                start_pos: [start.x, start.y, start.z],
                end_pos: [end.x, end.y, end.z],
                color: [color.x, color.y, color.z],
                shape_type: 1,
                radius: *radius,
                _padding: [0.0, 0.0, 0.0, 0.0, 0.0],
            },
        }
    }
}

struct BindGroupBuilder<'a> {
    layout_entries: Vec<BindGroupLayoutEntry>,
    entries: Vec<BindGroupEntry<'a>>,
    buffers: Vec<Buffer>,
}

impl<'a> BindGroupBuilder<'a> {
    fn new() -> Self {
        Self {
            layout_entries: Vec::new(),
            entries: Vec::new(),
            buffers: Vec::new(),
        }
    }

    fn add_buffer(&self, device: &Device, label: &str, data: &[u8], is_storage: bool) -> Self {
        let usage = if is_storage {
            BufferUsages::STORAGE
        } else {
            BufferUsages::UNIFORM
        };

        let binding_type = if is_storage {
            BufferBindingType::Storage { read_only: true }
        } else {
            BufferBindingType::Uniform
        };

        let binding = self.layout_entries.len() as u32;

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(label),
            contents: data,
            usage: usage | BufferUsages::COPY_DST,
        });

        let mut layout_entries = self.layout_entries.clone();
        layout_entries.push(BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: binding_type,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        let mut entries = self.entries.clone();
        entries.push(BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        });

        let mut buffers = self.buffers.clone();
        buffers.push(buffer);

        Self {
            buffers,
            entries,
            layout_entries,
        }
    }

    fn build(&self, device: &Device) -> (BindGroupLayout, BindGroup, Vec<Buffer>) {
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Main bind group layout"),
            entries: &self.layout_entries,
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Vertex shader bind group"),
            layout: &bind_group_layout,
            entries: &self.entries,
        });

        (bind_group_layout, bind_group, self.buffers)
    }
}

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    render_pipeline: RenderPipeline,
    buffers: Vec<Buffer>,
    msaa_texture: TextureView, // for antialiasing

    // `surface` should be the last to get dropped
    surface: Surface<'static>,
    surface_format: TextureFormat,
}

impl State {
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

        let shader_source = State::load_shader_source().unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let msaa_texture = State::create_msaa_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
        );

        let placeholder_4x4_matrix = [0.0f32; 64];
        let (bind_group_layout, bind_group, buffers) = BindGroupBuilder::new()
            .add_buffer(
                &device,
                "Camera matrix uniform buffer",
                bytemuck::cast_slice(&placeholder_4x4_matrix),
                false,
            )
            .build(&device);

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vertex_shader"),
                buffers: &[], // No vertex buffer, since a fullscreen quad is drawn in the vertex shader
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fragment_shader"),
                targets: &[Some(surface_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 4,
                ..MultisampleState::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let state = State {
            window,
            window_size,
            device,
            queue,
            render_pipeline,
            bind_group_layout,
            bind_group,
            buffers,
            msaa_texture,
            surface_format,
            surface,
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

    fn load_shader_source() -> Result<String, io::Error> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn create_msaa_texture(
        device: &Device,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> TextureView {
        let texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            label: Some("MSAA Texture"),
            view_formats: &[],
        });
        texture.create_view(&TextureViewDescriptor::default())
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.window_size = size;
        self.msaa_texture = State::create_msaa_texture(
            &self.device,
            self.surface_format.add_srgb_suffix(),
            self.window_size.width,
            self.window_size.height,
        );
        self.configure_surface();
    }

    pub fn get_window(&self) -> &Window {
        &self.window
    }

    fn update_uniforms(&self, camera_matrix: &[f32]) {}

    pub fn render(&mut self, camera_matrix: &[f32]) {
        self.update_uniforms(camera_matrix);

        let surface_texture = self.surface.get_current_texture().unwrap();
        let surface_texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
            format: Some(self.surface_format.add_srgb_suffix()),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Main render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.msaa_texture,
                    resolve_target: Some(&surface_texture_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1); // A ullscreen quad is being drawn in the vertex shader
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    controller: CameraController,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_resizable(true)
                        .with_title("Chemview"),
                )
                .unwrap(),
        );

        let state = pollster::block_on(State::new(window.clone()));

        self.state = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::RedrawRequested => {
                state.render(&self.controller.camera.padded_basis());
                state.get_window().request_redraw();
            }

            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => state.resize(size), // Resize will request a redraw

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: s,
                        ..
                    },
                ..
            } => {
                let pressed = s == ElementState::Pressed;

                match logical_key {
                    Key::Character(c) => match c.as_str() {
                        "w" => self.controller.set_action(Action::Forward, pressed),
                        "s" => self.controller.set_action(Action::Backward, pressed),
                        "a" => self.controller.set_action(Action::Left, pressed),
                        "d" => self.controller.set_action(Action::Right, pressed),
                        _ => {}
                    },

                    Key::Named(k) => match k {
                        NamedKey::ArrowDown => self.controller.set_action(Action::Down, pressed),
                        NamedKey::ArrowUp => self.controller.set_action(Action::Up, pressed),
                        _ => {}
                    },
                    _ => {}
                }

                state.get_window().request_redraw();
            }

            WindowEvent::MouseInput {
                state: ms, button, ..
            } => self
                .controller
                .set_mouse_pressed(button == MouseButton::Left && ms == ElementState::Pressed),

            WindowEvent::CursorMoved { position, .. } => {
                self.controller
                    .update_mouse_delta(position.x as f32, position.y as f32);
                state.get_window().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                self.controller.camera.zoom(delta_y < 0.0);
                state.get_window().request_redraw();
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
