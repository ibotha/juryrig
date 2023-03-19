use log::{error, info};
mod vulkan;
use winit::{
    event::{DeviceEvent, ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::{WindowBuilder, Fullscreen},
};

use crate::vulkan::Vulkan;

fn main() {
    pretty_env_logger::init_custom_env("KHODOL_LOG_LEVEL");

    info!("Logs initialised.");
    let mut vulkan: Option<Vulkan> = None;
    let event_loop = EventLoop::new();

    match WindowBuilder::new()
        .with_title("Rara se window")
        .with_fullscreen(Some(Fullscreen::Borderless(None)))
        .build(&event_loop)
    {
        Ok(window) => {
            event_loop.run(move |event, _, control_flow| {
                control_flow.set_poll();
                match event {
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    } => {
                        if window.id() == window_id {
                            control_flow.set_exit()
                        }
                    }
                    Event::DeviceEvent {
                        device_id: _device_id,
                        event,
                    } => {
                        // info!("Id: {:?}", _device_id);
                        match event {
                            DeviceEvent::Key(input) => {
                                // info!("Key Event {:?}", input.virtual_keycode.unwrap());
                                if input.state == ElementState::Pressed
                                    && input.virtual_keycode == Some(VirtualKeyCode::Escape)
                                {
                                    control_flow.set_exit()
                                }
                            }
                            _ => {}
                        }
                    }
                    Event::Resumed => {
                        //TODO: Initialise graphics context
                        info!("Ready to init graphics");
                        match Vulkan::new(&window) {
                            Ok(v) => {
                                vulkan = Some(v);
                            }
                            Err(e) => {
                                error!("Failed to initialise vulkan {:?}", e);
                                panic!("Could not initialise vulkan!");
                            }
                        }
                    }
                    Event::LoopDestroyed => {
                        // TODO: Destroy everything here
                        info!("Ready to destroy graphics");
                        vulkan.as_mut().unwrap().destroy();
                    }
                    Event::MainEventsCleared => {
                        // Draw Calls go here
                    }
                    _ => {}
                }
            })
        }
        Err(e) => {
            error!("Failed to initialise window. {}", e.to_string());
        }
    }
}
