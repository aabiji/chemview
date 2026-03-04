use crate::app::App;
use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod camera;
mod compound;
mod renderer;
mod shader;
mod shape;
mod ui;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
