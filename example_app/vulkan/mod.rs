use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::*,
    Device, Entry, Instance,
};
use log::{error, log, Level};
use std::ffi::{c_void, CStr, CString};
use winit::window::Window;

#[cfg(target_family = "windows")]
use {ash::extensions::khr::Win32Surface, winit::platform::windows::WindowExtWindows};

#[cfg(target_family = "unix")]
use {ash::extensions::khr::XlibSurface, winit::platform::unix::WindowExtUnix};

pub struct Vulkan {
    instance: Instance,
    surface_loader: Surface,
    surface: SurfaceKHR,
    logical_device: Device,
    graphics_queue: Queue,
    transfer_queue: Queue,
    debug_utils: DebugUtils,
    utils_messenger: DebugUtilsMessengerEXT,
    swapchain_loader: Swapchain,
    swapchain: SwapchainKHR,
}

impl Vulkan {
    pub fn new(window: &Window) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Entry::load() }?;
        
        // Layers and extentions
        let layer_names: Vec<CString> = vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
        let layer_name_pointers: Vec<*const i8> = layer_names
            .iter()
            .map(|layer_name| layer_name.as_ptr())
            .collect();
        let mut extension_name_pointers: Vec<*const i8> = vec![
            DebugUtils::name().as_ptr(),
            Surface::name().as_ptr()
        ];

        #[cfg(target_family = "windows")]
        extension_name_pointers.push(Win32Surface::name().as_ptr());

        #[cfg(target_family = "unix")]
        extension_name_pointers.push(XLibSurface::name().as_ptr());

        let mut debug_create_info = DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | DebugUtilsMessageSeverityFlagsEXT::INFO
                    | DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            )
            .pfn_user_callback(Some(Self::vulkan_debug_utils_callback));

        let instance = Self::create_instance(&entry, &mut debug_create_info, &layer_name_pointers, &extension_name_pointers)?;

        // Vulkan debugging
        let debug_utils = ash::extensions::ext::DebugUtils::new(&entry, &instance);

        let utils_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_create_info, None) }?;
        

        #[cfg(target_family = "windows")]
        fn create_surface(window: &Window, entry: &Entry, instance: &Instance) -> std::result::Result<SurfaceKHR, Result<>> {
            let hwnd = window.hwnd();
            let hinstance = window.hinstance();
            let win32_surface_create_info = Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd as *const c_void)
                .hinstance(hinstance as *const c_void);
            let win32_surface_loader = Win32Surface::new(&entry, &instance);
            return unsafe { win32_surface_loader.create_win32_surface(&win32_surface_create_info, None) };
        }

        #[cfg(target_family = "unix")]
        fn create_surface(window: &Window, entry: &Entry, instance: &Instance) -> std::result::Result<SurfaceKHR, Result<>> {
            let x11_display = window.xlib_display().unwrap();
            let x11_window = window.xlib_window().unwrap();
            let x11_create_info = vk::XlibSurfaceCreateInfoKHR::builder()
                .window(x11_window)
                .dpy(x11_display as *mut vk::Display);
            let xlib_surface_loader = ash::extensions::khr::XlibSurface::new(&entry, &instance);
            unsafe { xlib_surface_loader.create_xlib_surface(&x11_create_info, None) }
        }

        let surface = create_surface(&window, &entry, &instance)?;

        let surface_loader = Surface::new(&entry, &instance);

        let phys_devs = unsafe { instance.enumerate_physical_devices() }?;

        // Grab the last DISCRETE_GPU in the list of devices
        // TODO: Find a better way to choose which device to use.
        let (physical_device, physical_device_properties) = {
            let mut chosen = None;
            for p in phys_devs {
                let properties = unsafe { instance.get_physical_device_properties(p) };
                if properties.device_type == PhysicalDeviceType::DISCRETE_GPU {
                    chosen = Some((p, properties));
                }
            }
            chosen.unwrap()
        };

        let (graphics_queue_family_index, transfer_queue_family_index, queue_create_infos) = Self::determine_queues(&instance, physical_device, &surface_loader, surface)?;

        let (logical_device, graphics_queue, transfer_queue) =
            Self::create_logical_device(physical_device, &instance, &surface_loader, &layer_name_pointers, surface, graphics_queue_family_index, transfer_queue_family_index, &queue_create_infos)?;
        let (swapchain_loader, swapchain) = Self::create_swapchain(&logical_device, &instance, &surface_loader, surface, physical_device, graphics_queue_family_index)?;
        Ok(Self {
            instance,
            surface_loader,
            surface,
            logical_device,
            graphics_queue,
            transfer_queue,
            debug_utils,
            utils_messenger,
            swapchain_loader,
            swapchain
        })
    }

    pub fn destroy(&self) {
        unsafe {
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils
                .destroy_debug_utils_messenger(self.utils_messenger, None);
            self.instance.destroy_instance(None);
        };
    }
    
    fn create_instance(entry: &Entry, debug_create_info: &mut DebugUtilsMessengerCreateInfoEXT, layer_name_pointers: &Vec<*const i8>, extension_name_pointers: &Vec<*const i8>) -> std::result::Result<Instance, Box<dyn std::error::Error>> {
        
        let engine_name: CString = CString::new("Khodol").unwrap();
        let app_name: CString = CString::new(option_env!("CARGO_BIN_NAME").unwrap()).unwrap();
        let app_info = ApplicationInfo::builder()
            // This is the minimum Vulkan api version we are building for, newer versions have shinier
            // features but are not as widely available
            .api_version(make_api_version(0, 1, 0, 0))
            // This is information mainly used in crash logs and the like.
            // TODO: Get this name and version from whatever is using the engine.
            .application_name(&app_name)
            .application_version(make_api_version(
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
            .engine_name(&engine_name)
            .engine_version(make_api_version(
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

        // Instance creation
        let instance_create_info = InstanceCreateInfo::builder()
            .push_next(debug_create_info)
            .application_info(&app_info)
            .enabled_layer_names(&layer_name_pointers)
            .enabled_extension_names(&extension_name_pointers);

        let instance = unsafe { entry.create_instance(&instance_create_info, None) }?;
        Ok(instance)
    }

    unsafe extern "system" fn vulkan_debug_utils_callback(
        message_severity: DebugUtilsMessageSeverityFlagsEXT,
        message_type: DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
        _p_user_data: *mut c_void,
    ) -> Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
        let ty = format!("{:?}", message_type).to_lowercase();
        log!(
            match message_severity {
                DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Debug,
                DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
                DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
                DebugUtilsMessageSeverityFlagsEXT::WARNING => Level::Warn,
                _ => Level::Info,
            },
            "VK:{} {:?}",
            ty,
            message
        );
        FALSE
    }

    fn determine_queues(instance: &Instance, physical_device: PhysicalDevice, surface_loader: &Surface, surface: SurfaceKHR) -> std::result::Result<(u32,u32,Vec<DeviceQueueCreateInfo>), Box<dyn std::error::Error>>{
        // Choose our queue families for graphics and transfer.
        let queuefamilyproperties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let (graphics_queue_family_index, transfer_queue_family_index) = {
            let mut found_graphics_q_index = None;
            let mut found_transfer_q_index = None;
            let mut transfer_queue_specialization = 32;
            for (index, qfam) in queuefamilyproperties.iter().enumerate() {
                // TODO: Consider cases where the queue for dealing with a surface is different
                // from the queue that draws graphics
                if qfam.queue_count > 0
                    && qfam.queue_flags.contains(QueueFlags::GRAPHICS)
                    && unsafe {
                        surface_loader.get_physical_device_surface_support(
                            physical_device,
                            index as u32,
                            surface,
                        )
                    }?
                {
                    found_graphics_q_index = Some(index as u32);
                }
                if qfam.queue_count > 0
                    && qfam.queue_flags.intersects(
                        QueueFlags::TRANSFER | QueueFlags::GRAPHICS | QueueFlags::COMPUTE,
                    )
                {
                    let mut raw_queue_flags =
                        qfam.queue_flags.as_raw() & !QueueFlags::TRANSFER.as_raw();
                    let mut current_queue_specialisation = 0;

                    // We are trying to select the queue that is most specialized for the purpose of
                    // transferring so we count how many flags are set other than the transfer flag
                    while raw_queue_flags != 0 {
                        raw_queue_flags &= raw_queue_flags - 1;
                        current_queue_specialisation += 1;
                    }

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

        let priorities = [1.0f32];
        let multi_queue_priorities = [1.0f32, 1.0f32];
        // If both queues are the same we should merge them into the same family in the request
        let use_merged_queues = graphics_queue_family_index == transfer_queue_family_index;
        let queue_count = queuefamilyproperties[graphics_queue_family_index as usize].queue_count;

        let queue_infos = vec![
            DeviceQueueCreateInfo::builder()
                .queue_family_index(graphics_queue_family_index)
                .queue_priorities(&priorities)
                .build(),
            DeviceQueueCreateInfo::builder()
                .queue_family_index(transfer_queue_family_index)
                .queue_priorities(&priorities)
                .build(),
        ];
        let merged_queue_infos = vec![DeviceQueueCreateInfo::builder()
            .queue_family_index(graphics_queue_family_index)
            .queue_priorities(if queue_count > 1 {
                &multi_queue_priorities
            } else {
                &priorities
            })
            .build()];
        Ok((graphics_queue_family_index, transfer_queue_family_index, if use_merged_queues {
                merged_queue_infos
            } else {
                queue_infos
            }))
    }

    fn create_logical_device(
        physical_device: PhysicalDevice,
        instance: &Instance,
        surface_loader: &Surface,
        layer_name_pointers: &Vec<*const i8>,
        surface: SurfaceKHR,
        graphics_queue_family_index: u32,
        transfer_queue_family_index: u32,
        queue_infos: &Vec<DeviceQueueCreateInfo>
    ) -> std::result::Result<(ash::Device, Queue, Queue), Box<dyn std::error::Error>> {
        let device_extension_name_pointers: Vec<*const i8> =
            vec![Swapchain::name().as_ptr()];

        let device_create_info = DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extension_name_pointers)
            .enabled_layer_names(&layer_name_pointers);

        let logical_device =
            unsafe { instance.create_device(physical_device, &device_create_info, None) }?;

        let graphics_queue =
            unsafe { logical_device.get_device_queue(graphics_queue_family_index, 0) };
        // Todo: Use second queue if the family supports it
        let transfer_queue =
            unsafe { logical_device.get_device_queue(transfer_queue_family_index, 0) };

        Ok((logical_device, graphics_queue, transfer_queue))
    }

    fn create_swapchain(logical_device: &Device, instance: &Instance, surface_loader: &Surface, surface: SurfaceKHR, physical_device: PhysicalDevice, queue_index: u32) -> std::result::Result<(Swapchain, SwapchainKHR), Box<dyn std::error::Error>> {
        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?
        };
        let surface_present_modes = unsafe {
            surface_loader.get_physical_device_surface_present_modes(physical_device, surface)
        }?;
        let surface_formats =
            unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface) }?;

        let queuefamilies = [queue_index];

        let swapchain_create_info = SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(
                3.max(surface_capabilities.min_image_count)
                .min(surface_capabilities.max_image_count),
                )
            .image_format(surface_formats.first().unwrap().format)
            .image_color_space(surface_formats.first().unwrap().color_space)
            .image_extent(surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(SharingMode::EXCLUSIVE)
            .queue_family_indices(&queuefamilies)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(PresentModeKHR::FIFO);
        let swapchain_loader = Swapchain::new(&instance, &logical_device);
        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
        Ok((swapchain_loader, swapchain))
    }
}
