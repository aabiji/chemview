use glam::Vec3;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{WindowAttributes, WindowId},
};

use crate::compound::{Compound, CompoundShapes, ElementInfo};
use crate::renderer::Renderer;
use crate::{camera::Action, compound};

pub enum ViewType {
    BallAndStick,
    SpacingFilling,
}

pub struct AppState {
    pub compound: Compound,
    pub shapes: CompoundShapes,
    pub file_path: PathBuf,
    pub view_type: ViewType,
    pub wireframe_mode: bool,
    pub fps: f32,
}

impl AppState {
    fn default() -> Self {
        Self {
            compound: Compound::default(),
            shapes: CompoundShapes::default(),
            file_path: PathBuf::new(),
            view_type: ViewType::BallAndStick,
            wireframe_mode: false,
            fps: 0.0,
        }
    }

    fn load_compound(&mut self, camera_front: Vec3) -> Result<(), String> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path_str = self.file_path.file_name().unwrap().to_str().unwrap();
        let sdf_path = base.join(format!("data/{}", path_str).as_str());
        let info_path = base.join("data/element_data.json");

        let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
        let info: HashMap<String, ElementInfo> =
            serde_json::from_str(&contents).map_err(|err| err.to_string())?;

        let contents = std::fs::read_to_string(&sdf_path).map_err(|err| err.to_string())?;
        self.compound = compound::parse_compound(&contents)?;

        self.shapes = CompoundShapes::from(&self.compound, &info, camera_front, false);

        Ok(())
    }

    fn render_viewer_info(&mut self, ctx: &egui::Context) {
        egui::Window::new("Debug").show(ctx, |ui| {
            ui.label(format!("FPS: {}", self.fps));

            let response = ui.add(egui::TextEdit::singleline(&mut self.file_path));
            if response.changed() {
                // TODO: handle text change
            }

            ui.horizontal(|h_ui| {
                h_ui.heading(&format!("{}", self.compound.name));
                h_ui.heading(&format!("{}", self.compound.formula));
            });

            egui::ComboBox::from_label("Visualizer type")
                .selected_text(format!("{:?}", self.view_type))
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

#[derive(Default)]
pub struct App {
    renderer: Option<Renderer>,
    state: AppState,
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

        let mut renderer = pollster::block_on(Renderer::new(window.clone()));
        let group =
            compound::load_compound("chlorophyll_c", false, renderer.controller.front()).unwrap();
        renderer.set_shapes_data(group);

        self.renderer = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let renderer = self.renderer.as_mut().unwrap();

        let egui_consummed = renderer.ui.on_window_event(&renderer.window, &event);
        if egui_consummed {
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                renderer.render();
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

pub fn launch() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
