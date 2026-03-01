use egui_wgpu::{RendererOptions, ScreenDescriptor};
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::{event::WindowEvent, window::Window};

pub struct DebugUI {
    context: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

impl DebugUI {
    pub fn new(device: &Device, window: &Window, surface_format: TextureFormat) -> Self {
        let context = egui::Context::default();

        let state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let renderer = egui_wgpu::Renderer::new(
            device,
            surface_format,
            RendererOptions {
                msaa_samples: 1,
                dithering: false,
                predictable_texture_filtering: false,
                depth_stencil_format: None,
            },
        );

        Self {
            context,
            state,
            renderer,
        }
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    pub fn render(
        &mut self,
        device: &Device,
        window: &Window,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        surface_texture_view: &TextureView,
    ) {
        let raw_input = self.state.take_egui_input(&window);

        let full_output = self.context.run(raw_input, |ctx| self.render_ui(ctx));
        self.state
            .handle_platform_output(window, full_output.platform_output);

        let pixels_per_point = self.context.pixels_per_point();
        let clipped_primitives = self
            .context
            .tessellate(full_output.shapes, pixels_per_point);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [window.inner_size().width, window.inner_size().height],
            pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: surface_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // Preserve the existing pixels
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();

            self.renderer
                .render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    fn render_ui(&self, ctx: &egui::Context) {
        egui::SidePanel::left("Debug")
            .exact_width(200.0)
            .show(ctx, |ui| {
                ui.label("Moniker");
                ui.label("IUPAC: ");
                ui.label("Formula: ");
            });
    }
}
