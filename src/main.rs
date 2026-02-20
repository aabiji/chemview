use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_4;
use std::fs::File;
use std::io;
use std::mem::offset_of;
use std::path::PathBuf;
use std::sync::Arc;
use std::{borrow::Cow, io::Read};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferAddress, BufferBindingType, BufferSize,
    BufferUsages, DepthBiasState, DepthStencilState, Device, DeviceDescriptor, Extent3d,
    FragmentState, LoadOp, MultisampleState, Operations, PipelineLayoutDescriptor, PrimitiveState,
    Queue, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor,
    ShaderStages, StencilState, Surface, TextureDescriptor, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode, util::BufferInitDescriptor, util::DeviceExt,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceRaw {
    model_matrix: [[f32; 4]; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 4],
    tex_coord: [f32; 2],
}

fn create_vertex_data() -> (Vec<Vertex>, Vec<u16>) {
    let vertex = |x: i8, y: i8, z: i8, u: i8, v: i8| -> Vertex {
        Vertex {
            pos: [x as f32, y as f32, z as f32, 1.0],
            tex_coord: [u as f32, v as f32],
        }
    };

    let vertex_data = [
        // top (0, 0, 1)
        vertex(-1, -1, 1, 0, 0),
        vertex(1, -1, 1, 1, 0),
        vertex(1, 1, 1, 1, 1),
        vertex(-1, 1, 1, 0, 1),
        // bottom (0, 0, -1)
        vertex(-1, 1, -1, 1, 0),
        vertex(1, 1, -1, 0, 0),
        vertex(1, -1, -1, 0, 1),
        vertex(-1, -1, -1, 1, 1),
        // right (1, 0, 0)
        vertex(1, -1, -1, 0, 0),
        vertex(1, 1, -1, 1, 0),
        vertex(1, 1, 1, 1, 1),
        vertex(1, -1, 1, 0, 1),
        // left (-1, 0, 0)
        vertex(-1, -1, 1, 1, 0),
        vertex(-1, 1, 1, 0, 0),
        vertex(-1, 1, -1, 0, 1),
        vertex(-1, -1, -1, 1, 1),
        // front (0, 1, 0)
        vertex(1, 1, -1, 1, 0),
        vertex(-1, 1, -1, 0, 0),
        vertex(-1, 1, 1, 0, 1),
        vertex(1, 1, 1, 1, 1),
        // back (0, -1, 0)
        vertex(1, -1, 1, 0, 0),
        vertex(-1, -1, 1, 1, 0),
        vertex(-1, -1, -1, 1, 1),
        vertex(1, -1, -1, 0, 1),
    ];

    let index_data: &[u16] = &[
        0, 1, 2, 2, 3, 0, // top
        4, 5, 6, 6, 7, 4, // bottom
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}

fn create_instance_data() -> Vec<InstanceRaw> {
    let colors = [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    ];

    let mut instances = vec![];
    for y in 0..2 {
        for x in 0..2 {
            let scale = Vec3::ONE;
            let rotation = Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), FRAC_PI_4);
            let translation = Vec3::new(-1.2 + 3.0 * x as f32, -1.0 + 3.0 * y as f32, 0.0);
            let mat = Mat4::from_scale_rotation_translation(scale, rotation, translation);
            instances.push(InstanceRaw {
                model_matrix: mat.to_cols_array_2d(),
                color: colors[y * 2 + x].into(),
            });
        }
    }
    instances
}

fn load_shader_source() -> Result<String, io::Error> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

enum Translate {
    Up,
    Down,
    Left,
    Right,
    Forward,
    Backward,
}

struct Camera {
    pitch: f32,
    yaw: f32,
    field_of_view: f32,
    speed: f32,
    sensitivity: f32,
    prev_mouse_pos: Vec2,
    position: Vec3,
    front: Vec3,
}

impl Camera {
    fn new() -> Self {
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

    fn matrix(&self, aspect_ratio: f32) -> Mat4 {
        let fov = self.field_of_view.to_radians();
        let projection = Mat4::perspective_rh(fov, aspect_ratio, 1.0, 100.0);
        let view = Mat4::look_at_rh(self.position, self.position + self.front, Vec3::Y);
        projection * view
    }

    fn translate(&mut self, m: Translate) {
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

    fn rotate(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool) {
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

    fn zoom(&mut self, inwards: bool) {
        let offset = if inwards { -1.0 } else { 1.0 };
        self.field_of_view = (self.field_of_view + offset).clamp(1.0, 45.0);
    }
}

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    bind_group: BindGroup,
    render_pipeline: RenderPipeline,

    depth_texture_view: TextureView,
    msaa_texture_view: TextureView,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    uniform_buffer: Buffer,

    num_instances: u32,
    num_indices: u32,

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

        let (vertices, indices) = create_vertex_data();
        let num_indices = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        let camera = Camera::new();
        let ratio = window_size.width as f32 / window_size.height as f32;
        let mouse_down = false;
        let pressed_keys: HashSet<String> = HashSet::new();

        let uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Transformation matrix uniform buffer"),
            contents: bytemuck::cast_slice(camera.matrix(ratio).as_ref()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let instance_data = create_instance_data();
        let instance_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Instance buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        let num_instances = 4;

        let shader_source = load_shader_source().unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let depth_texture_view =
            State::create_depth_texture(&device, window_size.width, window_size.height);

        let msaa_texture_view = State::create_msaa_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
        );

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Main bind group layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Main bind group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: instance_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let vertex_buffers = [VertexBufferLayout {
            array_stride: size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: offset_of!(Vertex, pos) as u64,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: offset_of!(Vertex, tex_coord) as u64,
                    shader_location: 1,
                },
            ],
        }];

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vertex_shader"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fragment_shader"),
                targets: &[Some(surface_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
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
            bind_group,
            render_pipeline,

            depth_texture_view,
            msaa_texture_view,

            vertex_buffer,
            index_buffer,
            uniform_buffer,

            num_instances,
            num_indices,

            camera,
            mouse_down,
            pressed_keys,

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

    fn create_depth_texture(device: &Device, width: u32, height: u32) -> TextureView {
        let texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Depth24Plus,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TRANSIENT,
            label: Some("Depth buffer"),
            view_formats: &[],
        });
        texture.create_view(&TextureViewDescriptor::default())
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
        self.depth_texture_view = State::create_depth_texture(
            &self.device,
            self.window_size.width,
            self.window_size.height,
        );
        self.msaa_texture_view = State::create_msaa_texture(
            &self.device,
            self.surface_format.add_srgb_suffix(),
            self.window_size.width,
            self.window_size.height,
        );
        self.update_camera_transform();
        self.configure_surface();
    }

    fn update_camera_transform(&mut self) {
        let ratio = self.window_size.width as f32 / self.window_size.height as f32;
        let matrix = self.camera.matrix(ratio);
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(matrix.as_ref()),
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
                    view: &self.msaa_texture_view,
                    resolve_target: Some(&surface_texture_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw_indexed(0..self.num_indices, 0, 0..self.num_instances);
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
