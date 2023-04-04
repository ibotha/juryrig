use log::{error, info};
mod vulkan;
use winit::{
    event::{DeviceEvent, ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::{Fullscreen, WindowBuilder},
};

use crate::vulkan::Vulkan;

fn main() {
    pretty_env_logger::init_custom_env("JR_LOG_LEVEL");

    info!("Logs initialised.");
    let event_loop = EventLoop::new();

    match WindowBuilder::new()
        .with_title("Rara se window")
        .build(&event_loop)
    {
        Ok(window) => {
            let cached_window_id = window.id();
            let mut vulkan = Vulkan::new(window).expect("Could not init vulkan!");

            event_loop.run(move |event, _, control_flow| {
                control_flow.set_poll();
                match event {
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    } => {
                        if cached_window_id == window_id {
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
                        info!("Event-Startup");
                    }
                    Event::LoopDestroyed => {
                        // TODO: Destroy everything here
                        info!("Event-End");
                    }
                    Event::MainEventsCleared => {
                        // Event processing happens here
                        vulkan.window.request_redraw();
                    }
                    Event::RedrawRequested(_) => {
                        _ = vulkan.swap_framebuffers();
                    }
                    _ => {}
                }
            });
        }
        Err(e) => {
            error!("Failed to initialise window. {}", e.to_string());
        }
    }
}
