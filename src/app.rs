use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{WindowAttributes, WindowId},
};

use crate::camera::Action;
use crate::mmcif::MMCIFLoader;
use crate::pipeline::{CompoundPipeline, ViewType};
use crate::renderer::Renderer;
use crate::sdf::SDFLoader;
use crate::ui::UIState;

pub struct App {
    ui_state: UIState,
    renderer: Option<Renderer>,
    pipelines: HashMap<String, Box<dyn CompoundPipeline>>,
}

impl App {
    pub fn default() -> Self {
        Self {
            ui_state: UIState {
                file_path: String::from(""),
                path_changed: false,
                error_message: None,
                compound_description: String::new(),
                view_type: ViewType::BallAndStick,
                view_changed: false,
                fps: 0.0,
            },
            pipelines: HashMap::new(),
            renderer: None,
        }
    }

    fn update_compound(&mut self) {
        let mut load = || -> Result<(), String> {
            let extension = self
                .ui_state
                .file_path
                .split(".")
                .last()
                .ok_or("Unkonwn file format")?;

            // Memoize pipelines
            if !self.pipelines.contains_key(extension) {
                let obj: Box<dyn CompoundPipeline> = match extension {
                    "sdf" => Box::new(SDFLoader::init()?),
                    "cif" => Box::new(MMCIFLoader::init()?),
                    _ => return Err(String::from("Unkonwn file type")),
                };
                self.pipelines.insert(extension.to_string(), obj);
            }

            if self.ui_state.path_changed {
                let path = PathBuf::from(&self.ui_state.file_path);
                self.pipelines
                    .get_mut(extension)
                    .unwrap()
                    .parse_file(&path)?;
            }

            let front = self.renderer.as_mut().unwrap().controller.front();
            let mesh = self
                .pipelines
                .get_mut(extension)
                .unwrap()
                .compute_mesh_info(front, &self.ui_state.view_type);
            self.renderer.as_mut().unwrap().set_mesh_data(&mesh);

            Ok(())
        };

        if !self.ui_state.file_path.is_empty()
            && (self.ui_state.path_changed || self.ui_state.view_changed)
        {
            match load() {
                Ok(_) => self.ui_state.error_message = None,
                Err(err) => self.ui_state.error_message = Some(err),
            };
            self.ui_state.path_changed = false;
            self.ui_state.view_changed = false;
        }
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

        let egui_consummed = renderer.ui.on_window_event(&renderer.window, &event);
        if egui_consummed {
            renderer.get_window().request_redraw();
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                self.ui_state.fps = renderer.render(&mut self.ui_state);

                // Only update when needed
                if renderer.controller.is_active() {
                    renderer.get_window().request_redraw();
                }

                self.update_compound();
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
