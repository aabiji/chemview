use crate::app::App;
use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod camera;
mod mesh;
mod mmcif;
mod pipeline;
mod renderer;
mod sdf;
mod shader;
mod ui;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
