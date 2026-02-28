use bytemuck::offset_of;
use std::borrow::Cow;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use wgpu::{
    BindGroup, Buffer, BufferAddress, BufferUsages, DepthBiasState, DepthStencilState, Device,
    DeviceDescriptor, Extent3d, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, StencilState, Surface,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    util::BufferInitDescriptor, util::DeviceExt,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::shader::ShaderVar;
use crate::shape::{InstanceData, Shape, Vertex};
use crate::{
    camera::{Action, CameraController},
    compound,
};
use crate::{shader, shape};

// The maximum size in bytes of a storage buffer will be 10 MB
const STORAGE_BUFFE_SIZE: usize = 10 * 1024 * 1024;

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    render_pipeline: RenderPipeline,

    sphere_instance_range: Range<u32>,
    cylinder_instance_range: Range<u32>,
    sphere_index_range: Range<u32>,
    cylinder_index_range: Range<u32>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,

    bind_group: BindGroup,
    buffers: Vec<Buffer>,
    msaa_texture: TextureView,
    depth_texture: TextureView,

    controller: CameraController,

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

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
        let shader_source = shader::load_shader_source(&path).unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let (vertices, indices, sphere_index_range, cylinder_index_range) =
            shape::create_mesh_buffers(32, 32, 1.0, 2.0);

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

        let msaa_texture = State::create_msaa_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
        );

        let depth_texture =
            State::create_depth_texture(&device, window_size.width, window_size.height);

        let shader_vars = vec![
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 16,
                label: String::from("Projection matrix"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 16,
                label: String::from("View matrix"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 4,
                label: String::from("Camera position"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: true,
                num_bytes: STORAGE_BUFFE_SIZE,
                label: String::from("Instance data"),
            },
        ];
        let (buffers, bind_group_layout, bind_group) =
            shader::setup_shader_vars(&device, &shader_vars);

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
                bind_group_layouts: &[&bind_group_layout],
                immediate_size: 0,
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
            render_pipeline,

            sphere_index_range,
            cylinder_index_range,
            sphere_instance_range: 0..0,
            cylinder_instance_range: 0..0,
            vertex_buffer,
            index_buffer,

            bind_group,
            buffers,
            msaa_texture,
            depth_texture,

            controller: CameraController::new(),

            surface,
            surface_format,
        };
        state.configure_surface();
        state
    }

    fn create_msaa_texture(
        device: &Device,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> TextureView {
        // The texture will be used for antialiasing
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

    fn create_depth_texture(device: &Device, width: u32, height: u32) -> TextureView {
        let depth_texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Depth24Plus,
            usage: TextureUsages::RENDER_ATTACHMENT,
            label: Some("Depth buffer"),
            view_formats: &[],
        });
        depth_texture.create_view(&TextureViewDescriptor::default())
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

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.window_size = size;
        self.msaa_texture = State::create_msaa_texture(
            &self.device,
            self.surface_format.add_srgb_suffix(),
            size.width,
            size.height,
        );
        self.depth_texture = State::create_depth_texture(&self.device, size.width, size.height);
        self.configure_surface();
    }

    fn update_shader_vars(&mut self) {
        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` functio. Make sure they match!
        let ratio = (self.window_size.width as f32) / (self.window_size.height as f32);
        let (position, projection, view) = self.controller.camera_state(ratio);

        self.queue
            .write_buffer(&self.buffers[0], 0, bytemuck::cast_slice(&projection));

        self.queue
            .write_buffer(&self.buffers[1], 0, bytemuck::cast_slice(&view));

        self.queue
            .write_buffer(&self.buffers[2], 0, bytemuck::cast_slice(&position));
    }

    pub fn set_shapes_data(&mut self, shapes: Vec<Shape>) {
        let sphere_count = shapes
            .iter()
            .filter(|&s| matches!(s, Shape::Sphere { .. }))
            .count() as u32;
        self.sphere_instance_range = 0..sphere_count;
        self.cylinder_instance_range = sphere_count..shapes.len() as u32;

        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` functio. Make sure they match!
        let data: Vec<InstanceData> = shapes.iter().map(|s| shape::to_raw(s)).collect();
        let count = vec![shapes.len() as u32, 0u32, 0u32, 0u32];
        let shapes_raw = bytemuck::cast_slice(&data);

        assert!(shapes_raw.len() < STORAGE_BUFFE_SIZE); // TODO: handle error
        self.queue.write_buffer(&self.buffers[3], 0, shapes_raw); // Shapes data
    }

    fn render(&mut self) {
        self.controller.update_camera();
        self.update_shader_vars();

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
                multiview_mask: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);

            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            pass.draw_indexed(
                self.sphere_index_range.clone(),
                0,
                self.sphere_instance_range.clone(),
            );
            pass.draw_indexed(
                self.cylinder_index_range.clone(),
                0,
                self.cylinder_instance_range.clone(),
            );
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();
    }
}

#[derive(Default)]
pub struct App {
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

        let mut state = pollster::block_on(State::new(window.clone()));
        state.set_shapes_data(compound::load_compound("methane").unwrap());
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
                    Key::Character(c) => match c.as_str() {
                        "w" => state.controller.set_action(Action::Forward, pressed),
                        "s" => state.controller.set_action(Action::Backward, pressed),
                        "a" => state.controller.set_action(Action::Left, pressed),
                        "d" => state.controller.set_action(Action::Right, pressed),
                        _ => {}
                    },

                    Key::Named(k) => match k {
                        NamedKey::ArrowDown => state.controller.set_action(Action::Down, pressed),
                        NamedKey::ArrowUp => state.controller.set_action(Action::Up, pressed),
                        _ => {}
                    },

                    _ => {}
                }

                state.get_window().request_redraw();
            }

            WindowEvent::MouseInput {
                state: ms, button, ..
            } => state
                .controller
                .set_mouse_pressed(button == MouseButton::Left && ms == ElementState::Pressed),

            WindowEvent::CursorMoved { position, .. } => {
                state
                    .controller
                    .update_mouse_delta(position.x as f32, position.y as f32);
                state.get_window().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.controller.zoom(delta_y < 0.0);
                state.get_window().request_redraw();
            }

            _ => {}
        }
    }
}

pub fn launch() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
