use glam::Vec3;
use std::path::PathBuf;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{WindowAttributes, WindowId},
};

use crate::renderer::Renderer;
use crate::{camera::Action, compound};

#[derive(PartialEq)]
enum ViewType {
    BallAndStick,
    SpacingFilling,
}

impl ViewType {
    fn to_string(&self) -> String {
        match *self {
            ViewType::BallAndStick => String::from("Ball and Stick"),
            ViewType::SpacingFilling => String::from("Space filling"),
        }
    }
}

struct UIState {
    compound_formula: String,
    compound_name: String,
    file_path: String,
    error_message: String,
    handled_file_change: bool,
    view_type: ViewType,
    fps: f32,
}

impl UIState {
    fn render(&mut self, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .default_size([250.0, 250.0])
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|h_ui| {
                    h_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(">").clicked() {
                            self.handled_file_change = false;
                        }
                        ui.add_sized(
                            ui.available_size(),
                            egui::TextEdit::singleline(&mut self.file_path),
                        );
                    });
                });

                if !self.error_message.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("{}", self.error_message))
                            .color(egui::Color32::LIGHT_RED),
                    );
                }

                ui.horizontal(|h_ui| {
                    h_ui.label(egui::RichText::new(format!("{}", self.compound_name)).strong());
                    h_ui.add_space(45.0);
                    h_ui.label(egui::RichText::new(format!("{}", self.compound_formula)).strong());
                    h_ui.add_space(45.0);
                    h_ui.label(egui::RichText::new(format!("FPS: {}", self.fps)).strong());
                });

                ui.horizontal(|h_ui| {
                    h_ui.label("Visualizer type");
                    egui::ComboBox::from_id_salt("cobo")
                        .selected_text(self.view_type.to_string())
                        .show_ui(h_ui, |combo_ui| {
                            if combo_ui
                                .selectable_value(
                                    &mut self.view_type,
                                    ViewType::BallAndStick,
                                    ViewType::BallAndStick.to_string(),
                                )
                                .clicked()
                            {
                                self.handled_file_change = false;
                            }
                            if combo_ui
                                .selectable_value(
                                    &mut self.view_type,
                                    ViewType::SpacingFilling,
                                    ViewType::SpacingFilling.to_string(),
                                )
                                .clicked()
                            {
                                self.handled_file_change = false;
                            }
                        });
                });
            });
    }
}

pub struct App {
    renderer: Option<Renderer>,
    state: UIState,
}

impl App {
    pub fn default() -> Self {
        Self {
            state: UIState {
                compound_formula: String::new(),
                compound_name: String::new(),
                file_path: String::from(""),
                error_message: String::new(),
                view_type: ViewType::BallAndStick,
                handled_file_change: true,
                fps: 0.0,
            },
            renderer: None,
        }
    }

    fn load_compound(&mut self, camera_front: Vec3) -> Result<(), String> {
        let info = compound::load_element_info()?;
        let contents =
            std::fs::read_to_string(&self.state.file_path).map_err(|err| err.to_string())?;

        let (name, formula, atoms, bonds) = compound::parse_compound(&contents)?;
        let mesh = compound::assemble_mesh(
            atoms,
            bonds,
            &info,
            camera_front,
            self.state.view_type == ViewType::SpacingFilling,
        );

        self.renderer.as_mut().unwrap().set_mesh_data(&mesh);
        self.state.compound_name = name;
        self.state.compound_formula = formula;

        Ok(())
    }

    fn handle_new_compound(&mut self) {
        if self.state.handled_file_change {
            return;
        }

        let front = self.renderer.as_mut().unwrap().controller.front();
        if let Err(err) = self.load_compound(front) {
            self.state.error_message = err;
        } else {
            self.state.error_message = String::new();
        }
        self.state.handled_file_change = true;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_resizable(true)
                        .with_maximized(true)
                        .with_title("Chemview"),
                )
                .unwrap(),
        );

        let renderer = pollster::block_on(Renderer::new(window.clone()));
        self.renderer = Some(renderer);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let renderer = self.renderer.as_mut().unwrap();
        let mut callback = |ctx: &egui::Context| self.state.render(ctx);

        let egui_consummed = renderer.ui.on_window_event(&renderer.window, &event);
        if egui_consummed {
            renderer.get_window().request_redraw();
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                self.state.fps = renderer.render(&mut callback);

                // Only update when needed
                if renderer.controller.is_active() {
                    renderer.get_window().request_redraw();
                }

                self.handle_new_compound();
            }

            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => renderer.resize(size), // Resize will request a redraw

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
                        "w" => renderer.controller.set_action(Action::Forward, pressed),
                        "s" => renderer.controller.set_action(Action::Backward, pressed),
                        "a" => renderer.controller.set_action(Action::Left, pressed),
                        "d" => renderer.controller.set_action(Action::Right, pressed),
                        _ => {}
                    },

                    Key::Named(k) => match k {
                        NamedKey::ArrowDown => {
                            renderer.controller.set_action(Action::Down, pressed)
                        }
                        NamedKey::ArrowUp => renderer.controller.set_action(Action::Up, pressed),
                        _ => {}
                    },

                    _ => {}
                }

                renderer.get_window().request_redraw();
            }

            WindowEvent::MouseInput {
                state: ms, button, ..
            } => renderer
                .controller
                .set_mouse_pressed(button == MouseButton::Left && ms == ElementState::Pressed),

            WindowEvent::CursorMoved { position, .. } => {
                renderer
                    .controller
                    .update_mouse_delta(position.x as f32, position.y as f32);
                renderer.get_window().request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                renderer.controller.zoom(delta_y < 0.0);
                renderer.get_window().request_redraw();
            }

            _ => {}
        }
    }
}
