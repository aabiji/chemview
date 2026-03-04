use glam::Vec3;
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

struct UIState {
    compound_formula: String,
    compound_name: String,
    file_path: String,
    view_type: ViewType,
    wireframe_mode: bool,
    fps: f32,
}

impl UIState {
    fn render(&mut self, ctx: &egui::Context) {
        egui::Window::new("Debug").show(ctx, |ui| {
            ui.label(format!("FPS: {}", self.fps));

            let response = ui.add(egui::TextEdit::singleline(&mut self.file_path));
            if response.changed() {
                // TODO: handle text change
            }

            ui.horizontal(|h_ui| {
                h_ui.heading(&format!("{}", self.compound_name));
                h_ui.heading(&format!("{}", self.compound_formula));
            });

            egui::ComboBox::from_label("Visualizer type")
                .selected_text("Selected")
                .show_ui(ui, |combo_ui| {
                    combo_ui.selectable_value(
                        &mut self.view_type,
                        ViewType::BallAndStick,
                        "Ball and stick",
                    );
                    combo_ui.selectable_value(
                        &mut self.view_type,
                        ViewType::SpacingFilling,
                        "Space filling",
                    );
                });

            ui.checkbox(&mut self.wireframe_mode, "Wireframe mode");
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
                file_path: String::from("/home/aabiji/dev/chemview/data/dopamine.sdf"),
                view_type: ViewType::BallAndStick,
                wireframe_mode: false,
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
        let front = renderer.controller.front();
        self.renderer = Some(renderer);
        self.load_compound(front).unwrap();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let renderer = self.renderer.as_mut().unwrap();
        let mut callback = |ctx: &egui::Context| self.state.render(ctx);

        let egui_consummed = renderer.ui.on_window_event(&renderer.window, &event);
        if egui_consummed {
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                renderer.render(&mut callback);
                renderer.get_window().request_redraw();
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
