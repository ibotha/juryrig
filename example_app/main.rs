// #![allow(
//     dead_code,
//     unused_variables,
//     clippy::too_many_arguments,
//     clippy::unnecessary_wraps
// )]

// pub mod vulkan;
// use vulkanalia::prelude::v1_0::*;

// use winit::dpi::LogicalSize;
// use winit::event::{Event, WindowEvent};
// use winit::event_loop::{ControlFlow, EventLoop};
// use winit::window::WindowBuilder;
use khodol::init_window;
fn main() {
    pretty_env_logger::init();

    init_window("My Window");
}
