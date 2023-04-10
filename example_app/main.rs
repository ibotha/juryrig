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
            let cached_window_id = window.id().clone();
            let mut vulkan: Option<Vulkan> = None;

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
                    Event::WindowEvent {
                        event: WindowEvent::KeyboardInput { input, .. },
                        ..
                    } => {
                        if let winit::event::KeyboardInput {
                            state: winit::event::ElementState::Pressed,
                            virtual_keycode: Some(keycode),
                            ..
                        } = input
                        {
                            match &mut vulkan {
                                Some(v) => match keycode {
                                    winit::event::VirtualKeyCode::Right => {
                                        v.camera.turn_right(0.1);
                                    }
                                    winit::event::VirtualKeyCode::Left => {
                                        v.camera.turn_left(0.1);
                                    }
                                    winit::event::VirtualKeyCode::Up => {
                                        v.camera.move_forward(0.05);
                                    }
                                    winit::event::VirtualKeyCode::Down => {
                                        v.camera.move_backward(0.05);
                                    }
                                    winit::event::VirtualKeyCode::PageUp => {
                                        v.camera.turn_up(0.02);
                                    }
                                    winit::event::VirtualKeyCode::PageDown => {
                                        v.camera.turn_down(0.02);
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
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
                        vulkan = Some(Vulkan::new(&window).expect("Could not init vulkan!"))
                    }
                    Event::LoopDestroyed => {
                        // TODO: Destroy everything here
                        info!("Event-End");
                        vulkan = None
                    }
                    Event::MainEventsCleared => {
                        // Event processing happens here
                        window.request_redraw();
                    }
                    Event::RedrawRequested(_) => match &mut vulkan {
                        Some(v) => match v.swap_framebuffers() {
                            Err(e) => {
                                error!("Could not render frame! {:?}", e)
                            }
                            Ok(a) => {}
                        },
                        None => {}
                    },

                    _ => {}
                }
            });
        }
        Err(e) => {
            error!("Failed to initialise window. {}", e.to_string());
        }
    }
}
