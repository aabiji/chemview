use glam::Vec3;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use wgpu::{
    BindGroup, Buffer, Device, DeviceDescriptor, Extent3d, FragmentState, MultisampleState,
    PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions,
    ShaderModuleDescriptor, Surface, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, VertexState,
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
mod shader;
use crate::camera::{Action, CameraController};
use crate::shader::{RawShape, ShaderVar, Shape};

// The maximum size in bytes of a storage buffer will be 10 mb.
const STORAGE_BUFFE_SIZE: usize = 10 * 1024 * 1024;

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,

    device: Device,
    queue: Queue,
    render_pipeline: RenderPipeline,

    bind_group: BindGroup,
    buffers: Vec<Buffer>,
    msaa_texture: TextureView,

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

        let msaa_texture = State::create_msaa_texture(
            &device,
            surface_format.add_srgb_suffix(),
            window_size.width,
            window_size.height,
        );

        let shader_vars = vec![
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 16,
                label: String::from("View matrix"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: true,
                num_bytes: STORAGE_BUFFE_SIZE,
                label: String::from("Shapes data"),
            },
            ShaderVar {
                is_f32: false,
                is_storage: false,
                num_bytes: 4,
                label: String::from("Shape count"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 4,
                label: String::from("Resolution"),
            },
            ShaderVar {
                is_f32: true,
                is_storage: false,
                num_bytes: 4,
                label: String::from("Camera position"),
            },
        ];
        let (buffers, bind_group_layout, bind_group) =
            shader::setup_shader_vars(&device, &shader_vars);

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
            bind_group,
            buffers,
            msaa_texture,
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
            self.window_size.width,
            self.window_size.height,
        );
        self.configure_surface();
    }

    fn set_shapes_data(&mut self, shapes: Vec<Shape>) {
        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` functio. Make sure they match!
        let data: Vec<RawShape> = shapes.iter().map(|s| s.to_raw()).collect();
        let count = vec![shapes.len() as u32, 0u32, 0u32, 0u32];

        let shapes_raw = bytemuck::cast_slice(&data);
        let count_raw = bytemuck::cast_slice(&count);

        assert!(shapes_raw.len() < STORAGE_BUFFE_SIZE); // TODO: handle error
        self.queue.write_buffer(&self.buffers[1], 0, shapes_raw); // Shapes data
        self.queue.write_buffer(&self.buffers[2], 0, count_raw); // Shape count
    }

    fn update_shader_vars(&mut self) {
        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` functio. Make sure they match!
        let (position, matrix) = self.controller.camera_state();

        // View matrix
        self.queue
            .write_buffer(&self.buffers[0], 0, bytemuck::cast_slice(&matrix));

        // Camera position
        self.queue
            .write_buffer(&self.buffers[4], 0, bytemuck::cast_slice(&position));

        // Window resolution
        self.queue.write_buffer(
            &self.buffers[3],
            0,
            bytemuck::cast_slice(&[
                self.window_size.width as f32,
                self.window_size.height as f32,
                0.0,
                0.0,
            ]),
        );
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

        let mut state = pollster::block_on(State::new(window.clone()));
        state.set_shapes_data(vec![
            Shape::Sphere {
                origin: Vec3::new(3.0, 1.0, 1.0),
                color: Vec3::new(0.0, 1.0, 0.0),
                radius: 0.9,
            },
            Shape::Sphere {
                origin: Vec3::new(0.0, 0.0, 0.0),
                color: Vec3::new(1.0, 0.0, 0.0),
                radius: 1.0,
            },
            Shape::Sphere {
                origin: Vec3::new(0.0, 3.0, 0.0),
                color: Vec3::new(1.0, 1.0, 0.0),
                radius: 0.23,
            },
            Shape::Sphere {
                origin: Vec3::new(1.0, 0.0, 2.0),
                color: Vec3::new(0.0, 0.0, 1.0),
                radius: 0.5,
            },
        ]);
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

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
