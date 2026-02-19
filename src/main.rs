use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::f32::consts::FRAC_PI_4;
use std::fs::File;
use std::io;
use std::mem::offset_of;
use std::path::PathBuf;
use std::sync::Arc;
use std::{borrow::Cow, io::Read};
use wgpu::BindGroup;
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingType, Buffer, BufferAddress, BufferBindingType, BufferSize, BufferUsages, Device,
    DeviceDescriptor, FragmentState, MultisampleState, PipelineLayoutDescriptor, PrimitiveState,
    Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderStages, Surface,
    TextureFormat, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode, util::BufferInitDescriptor,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

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

fn load_shader_source() -> Result<String, io::Error> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shader.wgsl");
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn create_transform_matrix(aspect_ratio: f32) -> Mat4 {
    let projection = Mat4::perspective_rh(FRAC_PI_4, aspect_ratio, 1.0, 10.0);
    // The camera is point in the -z direction (out of the screen)
    // The cube is positioned at (0, 0, 0) for now, so the camera's z should be negative (behind)
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 6.0), Vec3::ZERO, Vec3::Y);
    projection * view
}

struct State {
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,
    device: Device,
    queue: Queue,
    bind_group: BindGroup,
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,
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

        let matrix = create_transform_matrix(window_size.width as f32 / window_size.height as f32);
        let uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Transformation matrix uniform buffer"),
            contents: bytemuck::cast_slice(matrix.as_ref()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let shader_source = load_shader_source().unwrap();
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Main shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source)),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Main bind group layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(64),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Main bind group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
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
            depth_stencil: None,
            multisample: MultisampleState::default(),
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
            vertex_buffer,
            index_buffer,
            num_indices,
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

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.window_size = size;
        self.configure_surface();
    }

    fn render(&mut self) {
        let surface_texture = self.surface.get_current_texture().unwrap();
        let texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
            format: Some(self.surface_format.add_srgb_suffix()),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Main render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &texture_view,
                    depth_slice: None,
                    resolve_target: None,
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
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw_indexed(0..self.num_indices, 0, 0..1);
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
            WindowEvent::Resized(size) => state.resize(size),
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
