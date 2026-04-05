use bytemuck::offset_of;
use glam::Vec3;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use wgpu::{
    BindGroup, Buffer, BufferAddress, BufferUsages, CommandEncoder, DepthBiasState,
    DepthStencilState, Device, DeviceDescriptor, Extent3d, FragmentState, LoadOp, MultisampleState,
    Operations, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, StencilState, Surface,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::CameraController;
use crate::shader;
use crate::shape::{RawShape, Shape, Vertex};
use crate::tessellate::TesselateOutput;
use crate::ui::{DebugUI, UIState};

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

    raytrace_pipeline: RenderPipeline,
    geometry_pipeline: RenderPipeline,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    shader_vars: Vec<Buffer>,
    bind_group: BindGroup,
    num_indices: u32,

    msaa_texture: TextureView,
    depth_texture: TextureView,

    pub ui: DebugUI,
    pub controller: CameraController,
    current_time: SystemTime,

    // `surface` should be the last to get dropped
    surface: Surface<'static>,
    surface_format: TextureFormat,
}

impl Renderer {
    /*
    Shapes such as spheres and cylinders are rendered using raytraceing done in the fragment shader,
    while curves are rendred using vertices and indices. This is because curves can't be instanced.
    So there are two rendering pipelines and two shaders, one for raytraceing and one for geometry rendering.
    */
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
        let (bind_group_layout, bind_group, shader_vars) = shader::create_shader_vars(&device);

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

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Geometry vertex buffer"),
            contents: &[0; 4], // dummy data
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Geometry index buffer"),
            contents: &[0; 4], // dummy data
            usage: BufferUsages::INDEX,
        });

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
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: offset_of!(Vertex, color) as u64,
                    shader_location: 2,
                },
            ],
        }];

        let raytrace_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("raytrace pipeline"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Raytrace pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            })),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("raytrace_vertex_shader"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("raytrace_fragment_shader"),
                targets: &[Some(surface_format.add_srgb_suffix().into())],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            multisample: MultisampleState {
                count: MSAA_SAMPLE_COUNT,
                ..MultisampleState::default()
            },
            cache: None,
            multiview: None,
            depth_stencil: None,
        });

        let geometry_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Geometry pipeline"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("geometry pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            })),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("geometry_vertex_shader"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("geometry_fragment_shader"),
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
            raytrace_pipeline,
            geometry_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices: 0,
            shader_vars,
            bind_group,
            msaa_texture,
            depth_texture,
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

    fn update_shader_vars(&mut self) {
        // NOTE: the indexes into self.buffer are taken from the order in which the shader
        // vars are defined in the `new` function. Make sure they match!
        let ratio = (self.window_size.width as f32) / (self.window_size.height as f32);
        let (position, projection, view, object_rotation) = self.controller.camera_state(ratio);
        let resolution = [self.window_size.width, self.window_size.height];

        self.queue
            .write_buffer(&self.shader_vars[0], 0, bytemuck::cast_slice(&projection));

        self.queue
            .write_buffer(&self.shader_vars[1], 0, bytemuck::cast_slice(&view));

        self.queue
            .write_buffer(&self.shader_vars[2], 0, bytemuck::cast_slice(&position));

        self.queue
            .write_buffer(&self.shader_vars[3], 0, bytemuck::cast_slice(&resolution));

        self.queue.write_buffer(
            &self.shader_vars[4],
            0,
            bytemuck::cast_slice(&object_rotation),
        );
    }

    pub fn set_mesh_data(&mut self, out: &TesselateOutput) {
        let target_pos = Vec3::new(0.0, 0.0, 0.0);
        let size = out.bounding_max - out.bounding_min;
        self.controller.fit_in_view(size);

        // Set shape data
        let offset = (out.bounding_min + size / 2.0) - target_pos;
        let shapes: Vec<RawShape> = out
            .shapes
            .iter()
            .map(|s: &Shape| s.translate(offset).raw())
            .collect();
        assert!(std::mem::size_of_val(&shapes) <= shader::STORAGE_BUFFER_SIZE);

        self.queue
            .write_buffer(&self.shader_vars[5], 0, bytemuck::cast_slice(&shapes));
        self.queue.write_buffer(
            &self.shader_vars[6],
            0,
            bytemuck::cast_slice(&[shapes.len() as u32]),
        );

        // Set geometry buffesr
        self.num_indices = out.indices.len() as u32;
        if self.num_indices > 0 {
            self.vertex_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Geometry vertex buffer"),
                contents: bytemuck::cast_slice(&out.vertices),
                usage: BufferUsages::VERTEX,
            });

            self.index_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Geometry index buffer"),
                contents: bytemuck::cast_slice(&out.indices),
                usage: BufferUsages::INDEX,
            });
        }
    }

    fn render_scene(
        &mut self,
        encoder: &mut CommandEncoder,
        surface_texture_view: &TextureView,
        delta_time: f32,
    ) {
        self.controller.update_camera(1.0 / delta_time);
        self.update_shader_vars();
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Geometry render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.msaa_texture,
                    resolve_target: None,
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

            pass.set_pipeline(&self.geometry_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("raytrace render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.msaa_texture,
                    resolve_target: Some(surface_texture_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.raytrace_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
    }

    pub fn render(&mut self, ui_state: &mut UIState) -> f32 {
        let now = SystemTime::now();
        let delta_time = now.duration_since(self.current_time).unwrap().as_millis() as f32;
        self.current_time = now;

        let surface_texture = self.surface.get_current_texture().unwrap();
        let surface_texture_view = surface_texture.texture.create_view(&TextureViewDescriptor {
            format: Some(self.surface_format),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.render_scene(&mut encoder, &surface_texture_view, delta_time);
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
