use std::ffi::{CStr, CString};

use ash::{
    extensions::{ext::DebugUtils, khr},
    vk, Device, Entry, Instance,
};
use winit::window::Window;

use super::surface::Surface;

fn validation_layer_name() -> &'static CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }
}

fn layer_name_pointers() -> Vec<*const i8> {
    vec![validation_layer_name().as_ptr()]
}

fn extension_name_pointers() -> Vec<*const i8> {
    let mut extension_name_pointers =
        vec![DebugUtils::name().as_ptr(), khr::Surface::name().as_ptr()];

    extension_name_pointers.push(Surface::extention_name_ptr());

    return extension_name_pointers;
}

pub fn create_instance(
    entry: &Entry,
    window: &Window,
    debug_create_info: &mut vk::DebugUtilsMessengerCreateInfoEXTBuilder,
) -> std::result::Result<Instance, vk::Result> {
    let engine_name: CString = CString::new("Juryrig").unwrap();
    let app_name: CString = CString::new(window.title().to_owned()).unwrap();

    // Layers and extentions

    let layer_name_pointers = layer_name_pointers();
    let extension_name_pointers = extension_name_pointers();

    let app_info = vk::ApplicationInfo::builder()
        // This is the minimum Vulkan api version we are building for, newer versions have shinier
        // features but are not as widely available
        .api_version(vk::make_api_version(0, 1, 1, 0))
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

pub fn init_physical_device_and_properties(
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

pub fn init_device_and_queues(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_families: &QueueFamilies,
) -> Result<(Device, Queues), vk::Result> {
    let layer_name_pointers = layer_name_pointers();

    let device_extension_name_pointers: Vec<*const i8> = vec![
        khr::Swapchain::name().as_ptr(),
        khr::BufferDeviceAddress::name().as_ptr(),
    ];

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

    let mut buffer_address_features =
        vk::PhysicalDeviceBufferDeviceAddressFeaturesKHR::builder().buffer_device_address(true);

    let enabled_features = vk::PhysicalDeviceFeatures::builder().sampler_anisotropy(true);

    let device_create_info = vk::DeviceCreateInfo::builder()
        .push_next(&mut buffer_address_features)
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&device_extension_name_pointers)
        .enabled_features(&enabled_features)
        .enabled_layer_names(&layer_name_pointers);

    let logical_device =
        unsafe { instance.create_device(physical_device, &device_create_info, None) }?;

    let graphics_queue =
        unsafe { logical_device.get_device_queue(queue_families.graphics_q_index.unwrap(), 0) };
    // Todo: Use second queue if the family supports it
    let transfer_queue =
        unsafe { logical_device.get_device_queue(queue_families.transfer_q_index.unwrap(), 0) };

    Ok((
        logical_device,
        Queues {
            graphics: graphics_queue,
            transfer: transfer_queue,
        },
    ))
}

pub struct QueueFamilies {
    pub graphics_q_index: Option<u32>,
    pub transfer_q_index: Option<u32>,
}

impl QueueFamilies {
    pub(super) fn new(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        surface: &Surface,
    ) -> Result<QueueFamilies, vk::Result> {
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

pub struct Queues {
    pub graphics: vk::Queue,
    pub transfer: vk::Queue,
}
