use ash::{
    extensions::{ext::DebugUtils, khr},
    vk, Device, Entry, Instance,
};
use log::{info, log, Level};
use std::ffi::{c_void, CStr, CString};
use winit::window::Window;

#[cfg(target_family = "windows")]
use {ash::extensions::khr::Win32Surface, winit::platform::windows::WindowExtWindows};

#[cfg(target_family = "unix")]
use {ash::extensions::khr::XlibSurface, winit::platform::unix::WindowExtUnix};

struct Debug {
    debug_utils: DebugUtils,
    utils_messenger: vk::DebugUtilsMessengerEXT,
}

impl Debug {
    fn new(
        entry: &Entry,
        instance: &Instance,
        debug_create_info: vk::DebugUtilsMessengerCreateInfoEXTBuilder,
    ) -> std::result::Result<Debug, vk::Result> {
        let debug_utils = DebugUtils::new(&entry, &instance);
        let utils_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_create_info, None) }?;
        Ok(Debug {
            debug_utils,
            utils_messenger,
        })
    }

    fn create_info() -> vk::DebugUtilsMessengerCreateInfoEXTBuilder<'static> {
        vk::DebugUtilsMessengerCreateInfoEXT::builder()
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
            .pfn_user_callback(Some(Self::vulkan_debug_utils_callback))
    }

    unsafe extern "system" fn vulkan_debug_utils_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _p_user_data: *mut c_void,
    ) -> vk::Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
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
}

impl Drop for Debug {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.utils_messenger, None)
        };
    }
}

struct Surface {
    loader: khr::Surface,
    surface: vk::SurfaceKHR,
}

impl Surface {
    fn new(window: &Window, entry: &Entry, instance: &Instance) -> Result<Surface, vk::Result> {
        fn create_surface(
            window: &Window,
            entry: &Entry,
            instance: &Instance,
        ) -> std::result::Result<vk::SurfaceKHR, vk::Result> {
            let hwnd = window.hwnd();
            let hinstance = window.hinstance();
            let win32_surface_create_info = vk::Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd as *const c_void)
                .hinstance(hinstance as *const c_void);
            let win32_surface_loader = khr::Win32Surface::new(&entry, &instance);
            return unsafe {
                win32_surface_loader.create_win32_surface(&win32_surface_create_info, None)
            };
        }

        #[cfg(target_family = "unix")]
        fn create_surface(
            window: &Window,
            entry: &Entry,
            instance: &Instance,
        ) -> std::result::Result<vk::SurfaceKHR, vk::Result> {
            let x11_display = window.xlib_display().unwrap();
            let x11_window = window.xlib_window().unwrap();
            let x11_create_info = vk::XlibSurfaceCreateInfoKHR::builder()
                .window(x11_window)
                .dpy(x11_display as *mut vk::Display);
            let xlib_surface_loader = khr::XlibSurface::new(&entry, &instance);
            unsafe { xlib_surface_loader.create_xlib_surface(&x11_create_info, None) }
        }

        let surface = create_surface(&window, &entry, &instance)?;

        let surface_loader = khr::Surface::new(&entry, &instance);
        Ok(Surface {
            surface,
            loader: surface_loader,
        })
    }

    fn get_physical_device_surface_support(&self, physical_device: vk::PhysicalDevice, queue_family_index: u32) -> Result<bool, vk::Result> {
        unsafe { self.loader.get_physical_device_surface_support(physical_device, queue_family_index, self.surface) }
    }

    pub(crate) fn get_capabilities(&self, physical_device: vk::PhysicalDevice) -> std::result::Result<vk::SurfaceCapabilitiesKHR, ash::vk::Result> {
        unsafe {
            self.loader.get_physical_device_surface_capabilities(physical_device, self.surface)}
    }

    pub(crate) fn get_present_modes(&self, physical_device: vk::PhysicalDevice) -> std::result::Result<Vec<vk::PresentModeKHR>, ash::vk::Result> {
        unsafe {
            self.loader.get_physical_device_surface_present_modes(physical_device, self.surface)
        }
    }

    pub(crate) fn get_formats(&self, physical_device: vk::PhysicalDevice) -> Result<Vec<vk::SurfaceFormatKHR>, vk::Result> {
        unsafe {
            self.loader.get_physical_device_surface_formats(physical_device, self.surface)
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.surface, None) }
    }
}

struct QueueFamilies {
    graphics_q_index: Option<u32>,
    transfer_q_index: Option<u32>,
}

impl QueueFamilies {
    fn new(instance: &Instance, physical_device: vk::PhysicalDevice, surface: &Surface) -> Result<QueueFamilies, vk::Result> {
        let queuefamilyproperties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

            let mut found_graphics_q_index = None;
            let mut found_transfer_q_index = None;
            let mut transfer_queue_specialization = 32;
            for (index, qfam) in queuefamilyproperties.iter().enumerate() {
                // TODO: Consider cases where the queue for dealing with a surface is different
                // from the queue that draws graphics
                if qfam.queue_count > 0
                    && qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && surface.get_physical_device_surface_support(physical_device, index as u32)?
                {
                    found_graphics_q_index = Some(index as u32);
                }
                if qfam.queue_count > 0
                    && qfam.queue_flags.intersects(
                        vk::QueueFlags::TRANSFER | vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE,
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

        Ok(QueueFamilies {
            graphics_q_index: found_graphics_q_index,
            transfer_q_index: found_transfer_q_index,
        })
    }
}

struct Queues {
    graphics: vk::Queue,
    transfer: vk::Queue,
}

struct Swapchain {
    loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    image_views: Vec<vk::ImageView>
}

impl Swapchain {
    fn init(instance: &Instance, physical_device: vk::PhysicalDevice, logical_device: &Device, surface: &Surface, queue_families: &QueueFamilies, queues: &Queues) -> Result<Swapchain, vk::Result> {
        let surface_capabilities = surface.get_capabilities(physical_device)?;
        let surface_present_modes = surface.get_present_modes(physical_device)?;
        let surface_formats = surface.get_formats(physical_device)?;
        dbg!(&surface_formats.first());
        let queuefamilies = [queue_families.graphics_q_index.unwrap()];

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface.surface)
            .min_image_count(
                3.max(surface_capabilities.min_image_count)
                    .min(surface_capabilities.max_image_count),
            )
            .image_format(vk::Format::R8G8B8A8_UNORM) //surface_formats.first().unwrap().format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR) //surface_formats.first().unwrap().color_space)
            .image_extent(surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queuefamilies)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO);
        let swapchain_loader = khr::Swapchain::new(&instance, &logical_device);
        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
        let mut image_views = Vec::with_capacity(swapchain_images.len());

        for image in &swapchain_images {
            let subresource_range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);
            let imageview_create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .subresource_range(*subresource_range);

            let imageview =
                unsafe { logical_device.create_image_view(&imageview_create_info, None) }?;

            image_views.push(imageview);
        };
        Ok(Swapchain { loader: swapchain_loader, swapchain, image_views })
    }

    unsafe fn cleanup(&self, logical_device: &Device) -> () {
       for iv in &self.image_views {
            logical_device.destroy_image_view(*iv, None);
        }
        self.loader
            .destroy_swapchain(self.swapchain, None) 
    }
}

pub struct Vulkan {
    instance: Instance,
    entry: Entry,
    pub window: Window,
    debug: std::mem::ManuallyDrop<Debug>,
    surface: std::mem::ManuallyDrop<Surface>,
    queue_families: QueueFamilies,
    logical_device: Device,
    queues: Queues,
    swapchain: Swapchain
}

impl Vulkan {
    fn validation_layer_name() -> &'static CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }
    }

    fn layer_name_pointers() -> Vec<*const i8> {
        vec![Self::validation_layer_name().as_ptr()]
    }

    fn extension_name_pointers() -> Vec<*const i8> {
        let mut extension_name_pointers =
            vec![DebugUtils::name().as_ptr(), khr::Surface::name().as_ptr()];

        #[cfg(target_family = "windows")]
        extension_name_pointers.push(Win32Surface::name().as_ptr());

        #[cfg(target_family = "unix")]
        extension_name_pointers.push(XLibSurface::name().as_ptr());
        return extension_name_pointers;
    }

    pub fn new(window: Window) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Entry::load() }?;

        let mut debug_create_info = Debug::create_info();

        let instance = Self::create_instance(&entry, &window, &mut debug_create_info)?;

        // Vulkan debugging
        let debug = Debug::new(&entry, &instance, debug_create_info)?;

        let surface: Surface = Surface::new(&window, &entry, &instance)?;

        let (physical_device, physical_device_properties) =
            Vulkan::init_physical_device_and_properties(&instance)?;

        let queue_families = QueueFamilies::new(&instance, physical_device, &surface)?;

        let (logical_device, queues) =
            Vulkan::init_device_and_queues(&instance, physical_device, &queue_families)?;

        let swapchain = Swapchain::init(
        &instance,
        physical_device,
        &logical_device,
        &surface,
        &queue_families,
        &queues,
    )?;
        
        Ok(Self {
            instance,
            entry,
            debug: std::mem::ManuallyDrop::new(debug),
            surface: std::mem::ManuallyDrop::new(surface),
            queue_families,
            logical_device,
            queues,
            swapchain,
            window
        })
    }

    fn create_instance(
        entry: &Entry,
        window: &Window,
        debug_create_info: &mut vk::DebugUtilsMessengerCreateInfoEXTBuilder,
    ) -> std::result::Result<Instance, vk::Result> {
        let engine_name: CString = CString::new("Khodol").unwrap();
        let app_name: CString = CString::new(window.title().to_owned()).unwrap();

        // Layers and extentions

        let layer_name_pointers = Self::layer_name_pointers();
        let extension_name_pointers = Self::extension_name_pointers();

        let app_info = vk::ApplicationInfo::builder()
            // This is the minimum Vulkan api version we are building for, newer versions have shinier
            // features but are not as widely available
            .api_version(vk::make_api_version(0, 1, 3, 0))
            // This is information mainly used in crash logs and the like.
            // TODO: Get this name and version from whatever is using the engine.
            .application_name(&app_name)
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
            .engine_name(&engine_name)
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

        // Instance creation
        let instance_create_info = vk::InstanceCreateInfo::builder()
            .push_next(debug_create_info)
            .application_info(&app_info)
            .enabled_layer_names(&layer_name_pointers)
            .enabled_extension_names(&extension_name_pointers);

        Ok(unsafe { entry.create_instance(&instance_create_info, None) }?)
    }

    fn init_physical_device_and_properties(
        instance: &Instance,
    ) -> Result<(vk::PhysicalDevice, vk::PhysicalDeviceProperties), vk::Result> {
        let phys_devs = unsafe { instance.enumerate_physical_devices() }?;

        // Grab the last DISCRETE_GPU in the list of devices
        // TODO: Find a better way to choose which device to use.
        let mut chosen = None;
        for p in phys_devs {
            let properties = unsafe { instance.get_physical_device_properties(p) };
            if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                chosen = Some((p, properties));
            }
        }
        Ok(chosen.unwrap())
    }

    fn init_device_and_queues(instance: &Instance, physical_device: vk::PhysicalDevice, queue_families: &QueueFamilies) -> Result<(Device, Queues), vk::Result> {
        let layer_name_pointers = Vulkan::layer_name_pointers();

        let device_extension_name_pointers: Vec<*const i8> = vec![khr::Swapchain::name().as_ptr()];

        let priorities = [1.0f32];
        let queue_infos = vec![
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_families.graphics_q_index.unwrap())
                .queue_priorities(&priorities)
                .build(),
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_families.transfer_q_index.unwrap())
                .queue_priorities(&priorities)
                .build(),
        ];

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extension_name_pointers)
            .enabled_layer_names(&layer_name_pointers);

        let logical_device =
            unsafe { instance.create_device(physical_device, &device_create_info, None) }?;

        let graphics_queue =
            unsafe { logical_device.get_device_queue(queue_families.graphics_q_index.unwrap(), 0) };
        // Todo: Use second queue if the family supports it
        let transfer_queue =
            unsafe { logical_device.get_device_queue(queue_families.transfer_q_index.unwrap(), 0) };

        Ok((logical_device, Queues { graphics: graphics_queue, transfer: transfer_queue }))
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        info!("Destroying vulkan");
        unsafe {
            self.swapchain.cleanup(&self.logical_device);
            self.logical_device.destroy_device(None);
            std::mem::ManuallyDrop::drop(&mut self.surface);
            std::mem::ManuallyDrop::drop(&mut self.debug);
            self.instance.destroy_instance(None);
        }
    }
}
