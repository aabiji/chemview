use egui_wgpu::{RendererOptions, ScreenDescriptor};
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::{event::WindowEvent, window::Window};

use crate::pipeline::ViewType;

pub struct UIState {
    pub file_path: String,
    pub path_changed: bool,
    pub error_message: Option<String>,
    pub compound_description: String,
    pub view_type: ViewType,
    pub view_changed: bool,
    pub fps: f32,
}

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

        context.set_style(egui::Style {
            text_styles: [
                (egui::TextStyle::Body, egui::FontId::proportional(15.0)),
                (egui::TextStyle::Button, egui::FontId::proportional(15.0)),
            ]
            .into(),
            ..Default::default()
        });

        Self {
            context,
            state,
            renderer,
        }
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    fn layout(&self, state: &mut UIState, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .default_size([250.0, 250.0])
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|h_ui| {
                    h_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(">").clicked() {
                            state.path_changed = true;
                        }
                        ui.add_sized(
                            ui.available_size(),
                            egui::TextEdit::singleline(&mut state.file_path),
                        );
                    });
                });

                if let Some(msg) = &state.error_message {
                    ui.label(egui::RichText::new(msg).color(egui::Color32::LIGHT_RED));
                }

                ui.horizontal(|h_ui| {
                    h_ui.label(egui::RichText::new(&state.compound_description).strong());
                    h_ui.add_space(45.0);
                    h_ui.label(egui::RichText::new(format!("FPS: {}", state.fps)).strong());
                });

                ui.horizontal(|h_ui| {
                    h_ui.label("Visualizer type");
                    egui::ComboBox::from_id_salt("combo")
                        .selected_text(state.view_type.to_string())
                        .show_ui(h_ui, |combo_ui| {
                            if combo_ui
                                .selectable_value(
                                    &mut state.view_type,
                                    ViewType::BallAndStick,
                                    ViewType::BallAndStick.to_string(),
                                )
                                .clicked()
                                || combo_ui
                                    .selectable_value(
                                        &mut state.view_type,
                                        ViewType::SpacingFilling,
                                        ViewType::SpacingFilling.to_string(),
                                    )
                                    .clicked()
                            {
                                state.view_changed = true;
                            }
                        });
                });
            });
    }

    pub fn render(
        &mut self,
        device: &Device,
        window: &Window,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        surface_texture_view: &TextureView,
        state: &mut UIState,
    ) {
        let raw_input = self.state.take_egui_input(window);

        let full_output = self.context.run(raw_input, |ctx| self.layout(state, ctx));
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
}
