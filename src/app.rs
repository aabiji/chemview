use glam::Vec3;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{WindowAttributes, WindowId},
};

use crate::loader::{FileLoader, MMCIFLoader, SDFLoader};
use crate::renderer::Renderer;
use crate::tessellate::{RenderStyle, TessellateOutput, Tessellator};
use crate::ui::UIState;
use crate::{camera::Action, tessellate::Structure};

enum Message {
    LoadFileRequest(PathBuf),
    TessRequest((RenderStyle, Vec3)),
    TessResponse(TessellateOutput),
    ErrResponse(String),
}

// Parse files and tessellate structures on a separate thread, as to not block the rendering thread.
fn run_loading_thread(rx_loader: Receiver<Message>, tx_app: Sender<Message>) {
    let mut loaders: HashMap<String, Box<dyn FileLoader>> = HashMap::new();
    let mut tessellator = Tessellator::new().unwrap();
    let mut structure = Structure::default();

    let mut handle_message = || -> Result<(), String> {
        match rx_loader.recv().map_err(|e| e.to_string())? {
            Message::LoadFileRequest(path) => {
                let extension = path
                    .extension()
                    .map(|s| s.to_str().unwrap())
                    .ok_or("Unknown file format")?;

                if !loaders.contains_key(extension) {
                    let obj: Box<dyn FileLoader> = match extension {
                        "sdf" => Box::new(SDFLoader {}),
                        "cif" => Box::new(MMCIFLoader::default()),
                        _ => return Err(String::from("Unknown file type")),
                    };
                    loaders.insert(extension.to_string(), obj);
                }

                structure = loaders.get_mut(extension).unwrap().parse_file(&path)?;
            }

            Message::TessRequest((view, front)) => {
                let structure = tessellator.tessellate(&structure, front, &view);
                let _ = tx_app.send(Message::TessResponse(structure));
            }

            _ => {}
        };

        Ok(())
    };

    loop {
        if let Err(e) = handle_message() {
            let _ = tx_app.send(Message::ErrResponse(e));
        }
    }
}

pub struct App {
    renderer: Option<Renderer>,
    ui_state: UIState,
    rx_app: Receiver<Message>,
    tx_loader: Sender<Message>,
    _loader_handler: JoinHandle<()>,
}

impl App {
    pub fn default() -> Self {
        let (tx_loader, rx_loader) = mpsc::channel::<Message>();
        let (tx_app, rx_app) = mpsc::channel::<Message>();
        let _loader_handler = thread::spawn(move || run_loading_thread(rx_loader, tx_app));

        Self {
            ui_state: UIState {
                file_path: String::from("/home/aabiji/dev/chemview/data/mmcif/T44.cif"),
                path_changed: false,
                error_message: None,
                view_type: RenderStyle::BallAndStick,
                view_changed: false,
                fps: 0.0,
            },
            renderer: None,
            tx_loader,
            rx_app,
            _loader_handler,
        }
    }

    fn update_compound(&mut self) {
        // Dispatch requests to the loading thread
        if self.ui_state.path_changed {
            let path = PathBuf::from(&self.ui_state.file_path);
            let _ = self.tx_loader.send(Message::LoadFileRequest(path));
            self.ui_state.path_changed = false;
            self.ui_state.view_changed = true;
        }

        if self.ui_state.view_changed {
            let front = self.renderer.as_mut().unwrap().controller.front();
            let _ = self
                .tx_loader
                .send(Message::TessRequest((self.ui_state.view_type, front)));
            self.ui_state.view_changed = false;
        }

        // Listen for responses
        if let Ok(msg) = self.rx_app.try_recv() {
            self.ui_state.error_message = None;

            match msg {
                Message::TessResponse(output) => {
                    self.renderer.as_mut().unwrap().set_mesh_data(&output);
                }
                Message::ErrResponse(e) => self.ui_state.error_message = Some(e),
                _ => {}
            }
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
