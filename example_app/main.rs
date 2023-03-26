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
        .with_fullscreen(Some(Fullscreen::Borderless(None)))
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
                        vulkan.swapchain.current_image = (vulkan.swapchain.current_image + 1)
                            % vulkan.swapchain.amount_of_images as usize;
                        let (image_index, _) = unsafe {
                            vulkan
                                .swapchain
                                .loader
                                .acquire_next_image(
                                    vulkan.swapchain.swapchain,
                                    std::u64::MAX,
                                    vulkan.swapchain.image_available
                                        [vulkan.swapchain.current_image],
                                    ash::vk::Fence::null(),
                                )
                                .expect("image acquisition trouble")
                        };
                        unsafe {
                            vulkan
                                .logical_device
                                .wait_for_fences(
                                    &[vulkan.swapchain.may_begin_drawing
                                        [vulkan.swapchain.current_image]],
                                    true,
                                    std::u64::MAX,
                                )
                                .expect("fence-waiting");

                            vulkan
                                .logical_device
                                .reset_fences(&[vulkan.swapchain.may_begin_drawing
                                    [vulkan.swapchain.current_image]])
                                .expect("resetting fences");
                        }
                        let semaphores_available =
                            [vulkan.swapchain.image_available[vulkan.swapchain.current_image]];
                        let waiting_stages = [ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                        let semaphores_finished =
                            [vulkan.swapchain.rendering_finished[vulkan.swapchain.current_image]];
                        let commandbuffers = [vulkan.command_buffers[image_index as usize]];
                        let submit_info = [ash::vk::SubmitInfo::builder()
                            .wait_semaphores(&semaphores_available)
                            .wait_dst_stage_mask(&waiting_stages)
                            .command_buffers(&commandbuffers)
                            .signal_semaphores(&semaphores_finished)
                            .build()];
                        unsafe {
                            vulkan
                                .logical_device
                                .queue_submit(
                                    vulkan.queues.graphics,
                                    &submit_info,
                                    vulkan.swapchain.may_begin_drawing
                                        [vulkan.swapchain.current_image],
                                )
                                .expect("queue submission");
                        };
                        let swapchains = [vulkan.swapchain.swapchain];
                        let indices = [image_index];
                        let present_info = ash::vk::PresentInfoKHR::builder()
                            .wait_semaphores(&semaphores_finished)
                            .swapchains(&swapchains)
                            .image_indices(&indices);
                        unsafe {
                            vulkan
                                .swapchain
                                .loader
                                .queue_present(vulkan.queues.graphics, &present_info)
                                .expect("queue presentation");
                        };
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
