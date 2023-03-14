use log::{info, error};
use winit::{event_loop::EventLoop, window::WindowBuilder, event::{Event, WindowEvent, DeviceEvent, ElementState, VirtualKeyCode}};

fn main() {
    pretty_env_logger::init();

    info!("Logs initialised.");

    let event_loop = EventLoop::new();

    match WindowBuilder::new().with_title("Rara se window").build(&event_loop) {
        Ok(window) => {
            event_loop.run(move |event, _, control_flow| {

                match event {
                    Event::WindowEvent { window_id, event: WindowEvent::CloseRequested } => {
                        if window.id() == window_id {
                            control_flow.set_exit()
                        }
                    }
                    Event::DeviceEvent { device_id: _device_id, event } => {
                        // info!("Id: {:?}", _device_id);
                        match event {
                            DeviceEvent::Key(input) => {
                                // info!("Key Event {:?}", input.virtual_keycode.unwrap());
                                if input.state == ElementState::Pressed && input.virtual_keycode == Some(VirtualKeyCode::Escape) {
                                    control_flow.set_exit()
                                }
                            }
                            _ => {}
                        }
                    }
                    Event::Resumed => {
                        //TODO: Initialise graphics context
                        info!("Ready to init graphics");
                        control_flow.set_wait()
                    }
                    Event::LoopDestroyed => {
                        // TODO: Destroy everything here
                        info!("Ready to destroy graphics");
                        control_flow.set_wait()
                    }
                    _ => {
                        control_flow.set_wait()
                    }
                }
            })
        }
        Err(e) => {
            error!("Failed to initialise window. {}", e.to_string());
        }
    }
}
