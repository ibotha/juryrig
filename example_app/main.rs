use core::panic;
use std::os::raw::c_void;

use ash::{vk, Entry};
use log::{error, info, log, Level};
use winit::{
    event::{DeviceEvent, ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
};

unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = std::ffi::CStr::from_ptr((*p_callback_data).p_message);
    let ty = format!("{:?}", message_type).to_lowercase();
    log!(
        match message_severity {
            vk::DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Debug,
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => Level::Warn,
            _ => Level::Info,
        },
        "VK:{} {:?}",
        ty,
        message
    );
    vk::FALSE
}

fn main() {
    pretty_env_logger::init_custom_env("KHODOL_LOG_LEVEL");

    info!("Logs initialised.");
    let entry = unsafe { Entry::load() }.unwrap_or_else(|e| {
        error!("Failed to load vulkan lib {}", e.to_string());
        panic!("Could not initialize vilkan");
    });

    let enginename = std::ffi::CString::new("Khodol").unwrap();
    let appname = std::ffi::CString::new("ExampleApp").unwrap();
    let app_info = vk::ApplicationInfo::builder()
        // This is the minimum Vulkan api version we are building for, newer versions have shinier
        // features but are not as widely available
        .api_version(vk::make_api_version(0, 1, 0, 0))
        // This is information mainly used in crash logs and the like.
        // TODO: Get this name and version from whatever is using the engine.
        .application_name(&appname)
        .application_version(vk::make_api_version(
            0,
            option_env!("CARGO_PKG_VERSION_MAJOR")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
            option_env!("CARGO_PKG_VERSION_MINOR")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
            option_env!("CARGO_PKG_VERSION_PATCH")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
        ))
        // Similar to the application stuff this is also mainly used for
        // debugging and logging purposes
        .engine_name(&enginename)
        .engine_version(vk::make_api_version(
            0,
            option_env!("CARGO_PKG_VERSION_MAJOR")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
            option_env!("CARGO_PKG_VERSION_MINOR")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
            option_env!("CARGO_PKG_VERSION_PATCH")
                .unwrap()
                .parse::<u32>()
                .unwrap(),
        ));

    // Layers and extentions
    let layer_names: Vec<std::ffi::CString> =
        vec![std::ffi::CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
    let layer_name_pointers: Vec<*const i8> = layer_names
        .iter()
        .map(|layer_name| layer_name.as_ptr())
        .collect();
    let extension_name_pointers: Vec<*const i8> = vec![
        ash::extensions::ext::DebugUtils::name().as_ptr(),
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::Win32Surface::name().as_ptr(),
    ];

    let mut debug_create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        )
        .pfn_user_callback(Some(vulkan_debug_utils_callback));

    // Instance creation
    let instance_create_info = vk::InstanceCreateInfo::builder()
        .push_next(&mut debug_create_info)
        .application_info(&app_info)
        .enabled_layer_names(&layer_name_pointers)
        .enabled_extension_names(&extension_name_pointers);

    let instance =
        unsafe { entry.create_instance(&instance_create_info, None) }.unwrap_or_else(|e| {
            error!("Failed to create vulkan instance {}", e.to_string());
            panic!("Could not create vulkan instance");
        });

    // Vulkan debugging
    let debug_utils = ash::extensions::ext::DebugUtils::new(&entry, &instance);

    let utils_messenger =
        unsafe { debug_utils.create_debug_utils_messenger(&debug_create_info, None) }
            .unwrap_or_else(|e| {
                error!("Failed to initialize vulkan debugging {}", e.to_string());
                panic!("Could not initialize vilkan debugging");
            });

    let event_loop = EventLoop::new();

    match WindowBuilder::new()
        .with_title("Rara se window")
        .build(&event_loop)
    {
        Ok(window) => {
            let hwnd = window.hwnd();
            let hinstance = window.hinstance();
            let win32_surface_create_info = vk::Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd as *const c_void)
                .hinstance(hinstance as *const c_void);
            let win32_surface_loader = ash::extensions::khr::Win32Surface::new(&entry, &instance);
            let surface = unsafe {
                win32_surface_loader.create_win32_surface(&win32_surface_create_info, None)
            }
            .unwrap_or_else(|e| {
                error!("Failed to create surface {:?}", e);
                panic!("Could not create surface");
            });
            let surface_loader = ash::extensions::khr::Surface::new(&entry, &instance);

            let phys_devs = unsafe { instance.enumerate_physical_devices() }.unwrap_or_else(|e| {
                error!("Failed to get physical devices {}", e.to_string());
                panic!("Could not get physical devices");
            });

            // Grab the last DISCRETE_GPU in the list of devices
            // TODO: Find a better way to choose which device to use.
            let (physical_device, physical_device_properties) = {
                let mut chosen = None;
                for p in phys_devs {
                    let properties = unsafe { instance.get_physical_device_properties(p) };
                    if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                        chosen = Some((p, properties));
                    }
                }
                chosen.unwrap()
            };

            // Choose our queue families for graphics and transfer.
            let queuefamilyproperties =
                unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
            dbg!(&queuefamilyproperties);
            let (graphics_queue_family_index, transfer_queue_family_index) = {
                let mut found_graphics_q_index = None;
                let mut found_transfer_q_index = None;
                let mut transfer_queue_specialization = 32;
                for (index, qfam) in queuefamilyproperties.iter().enumerate() {
                    // TODO: Consider cases where the queue for dealing with a surface is different
                    // from the queue that draws graphics
                    if qfam.queue_count > 0 && qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && unsafe {
                            surface_loader.get_physical_device_surface_support(physical_device, index as u32, surface)
                        }.unwrap_or_else(|e| {
                            error!("Failed to check physical device support for windows surface {}", e.to_string());
                            panic!("Could not verify support for graphics queue family");
                        })
                    {
                        found_graphics_q_index = Some(index as u32);
                    }
                    if qfam.queue_count > 0
                        && qfam.queue_flags.intersects(
                            vk::QueueFlags::TRANSFER
                                | vk::QueueFlags::GRAPHICS
                                | vk::QueueFlags::COMPUTE,
                        )
                    {
                        let mut raw_queue_flags =
                            qfam.queue_flags.as_raw() & !vk::QueueFlags::TRANSFER.as_raw();
                        let mut current_queue_specialisation = 0;

                        // We are trying to select the queue that is most specialized for the purpose of
                        // transferring so we count how many flags are set other than the transfer flag
                        while raw_queue_flags != 0 {
                            raw_queue_flags &= raw_queue_flags - 1;
                            current_queue_specialisation += 1;
                        }

                        info!(
                            "queue {} has {} flags set",
                            index, current_queue_specialisation
                        );

                        // Choose a new family if we either don't already have one, are currently using the
                        // same family as our graphics, or have found a more specialized queue
                        if found_transfer_q_index.is_none()
                            || found_transfer_q_index == found_graphics_q_index
                            || (current_queue_specialisation < transfer_queue_specialization)
                        {
                            found_transfer_q_index = Some(index as u32);
                            transfer_queue_specialization = current_queue_specialisation;
                        }
                    }
                }
                (
                    found_graphics_q_index.unwrap(),
                    found_transfer_q_index.unwrap(),
                )
            };
            info!(
                "Vulkan queue indices are: [graphics: {}, transfer: {}]",
                graphics_queue_family_index, transfer_queue_family_index
            );

            let priorities = [1.0f32];
            let multi_queue_priorities = [1.0f32, 1.0f32];
            // If both queues are the same we should merge them into the same family in the request
            let use_merged_queues = graphics_queue_family_index == transfer_queue_family_index;
            let queue_count =
                queuefamilyproperties[graphics_queue_family_index as usize].queue_count;

            let queue_infos = [
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(graphics_queue_family_index)
                    .queue_priorities(&priorities)
                    .build(),
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(transfer_queue_family_index)
                    .queue_priorities(&priorities)
                    .build(),
            ];
            let merged_queue_infos = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(graphics_queue_family_index)
                .queue_priorities(if queue_count > 1 {
                    &multi_queue_priorities
                } else {
                    &priorities
                })
                .build()];

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(if use_merged_queues {
                    &merged_queue_infos
                } else {
                    &queue_infos
                })
                .enabled_layer_names(&layer_name_pointers);
            let logical_device =
                unsafe { instance.create_device(physical_device, &device_create_info, None) }
                    .unwrap_or_else(|e| {
                        error!("Failed to create logical device {}", e.to_string());
                        panic!("Could not create logical device");
                    });

            let graphics_queue =
                unsafe { logical_device.get_device_queue(graphics_queue_family_index, 0) };
            // Todo: Use second queue if the family supports it
            let transfer_queue =
                unsafe { logical_device.get_device_queue(transfer_queue_family_index, 0) };
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
                    }
                    Event::LoopDestroyed => {
                        // TODO: Destroy everything here
                        info!("Ready to destroy graphics");
                        unsafe {
                            logical_device.destroy_device(None);
                            surface_loader.destroy_surface(surface, None);
                        }
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

    unsafe {
        debug_utils.destroy_debug_utils_messenger(utils_messenger, None);
        instance.destroy_instance(None);
    };
}
