use std::ffi::{CStr, CString};

use ash::{
    extensions::{ext::DebugUtils, khr},
    vk::{self, ExtDescriptorIndexingFn},
    Device, Entry, Instance,
};
use na::min;
use winit::window::Window;

use super::{error::InitError, surface::Surface};

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

pub(super) fn init_device_and_queues(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_families: &QueueFamilies,
) -> Result<(Device, Queues), vk::Result> {
    let layer_name_pointers = layer_name_pointers();

    let device_extension_name_pointers: Vec<*const i8> = vec![
        khr::Swapchain::name().as_ptr(),
        khr::BufferDeviceAddress::name().as_ptr(),
        ExtDescriptorIndexingFn::name().as_ptr(),
    ];

    let priorities: [&[f32]; 3] = [&[1.0f32], &[1.0f32, 1.0f32], &[1.0f32, 1.0f32, 1.0f32]];
    let mut queue_infos: Vec<vk::DeviceQueueCreateInfo> = vec![];
    if queue_families.graphics == queue_families.compute {
        if queue_families.compute == queue_families.transfer {
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.graphics)
                    .queue_priorities(
                        &priorities[min(queue_families.graphics_queue_count as usize - 1, 2)],
                    )
                    .build(),
            )
        } else {
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.graphics)
                    .queue_priorities(
                        &priorities[min(queue_families.graphics_queue_count as usize - 1, 1)],
                    )
                    .build(),
            );
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.transfer)
                    .queue_priorities(&priorities[0])
                    .build(),
            )
        }
    } else {
        if queue_families.compute == queue_families.transfer {
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.graphics)
                    .queue_priorities(&priorities[0])
                    .build(),
            );
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.compute)
                    .queue_priorities(
                        &priorities[min(queue_families.compute_queue_count as usize - 1, 1)],
                    )
                    .build(),
            )
        } else {
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.graphics)
                    .queue_priorities(&priorities[0])
                    .build(),
            );
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.compute)
                    .queue_priorities(&priorities[0])
                    .build(),
            );
            queue_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_families.transfer)
                    .queue_priorities(&priorities[0])
                    .build(),
            );
        }
    }

    let mut buffer_address_features =
        vk::PhysicalDeviceBufferDeviceAddressFeaturesKHR::builder().buffer_device_address(true);

    let mut indexing_features = vk::PhysicalDeviceDescriptorIndexingFeatures::builder()
        .runtime_descriptor_array(true)
        .descriptor_binding_variable_descriptor_count(true);

    let enabled_features = vk::PhysicalDeviceFeatures::builder().sampler_anisotropy(true);

    let device_create_info = vk::DeviceCreateInfo::builder()
        .push_next(&mut buffer_address_features)
        .push_next(&mut indexing_features)
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&device_extension_name_pointers)
        .enabled_features(&enabled_features)
        .enabled_layer_names(&layer_name_pointers);

    let logical_device =
        unsafe { instance.create_device(physical_device, &device_create_info, None) }?;

    // This is slightly convoluted but the upshot is this;
    // 1: We set the graphics queue
    // 2: If the compute queue family is the same as the graphics queue family then check if we have space to use separate queues
    //     otherwise, use the same queue as graphics.
    // 3: Try to allocate a standalone transfer queue, if that fails it will piggy_back off of the compute queue,
    //     failing that it will piggy back off the graphics queue.
    let graphics_queue = unsafe { logical_device.get_device_queue(queue_families.graphics, 0) };
    let compute_queue = if queue_families.compute == queue_families.graphics {
        if queue_families.graphics_queue_count > 1 {
            unsafe { logical_device.get_device_queue(queue_families.compute, 1) }
        } else {
            graphics_queue
        }
    } else {
        unsafe { logical_device.get_device_queue(queue_families.compute, 0) }
    };
    let transfer_queue = if queue_families.transfer == queue_families.graphics {
        if queue_families.graphics_queue_count > 2 {
            unsafe { logical_device.get_device_queue(queue_families.compute, 2) }
        } else {
            compute_queue
        }
    } else {
        if queue_families.transfer == queue_families.compute {
            if queue_families.compute_queue_count > 1 {
                unsafe { logical_device.get_device_queue(queue_families.compute, 1) }
            } else {
                compute_queue
            }
        } else {
            unsafe { logical_device.get_device_queue(queue_families.transfer, 0) }
        }
    };

    Ok((
        logical_device,
        Queues {
            graphics: graphics_queue,
            compute: compute_queue,
            transfer: transfer_queue,
        },
    ))
}

pub(super) struct QueueFamilies {
    pub(super) graphics_queue_count: u32,
    pub(super) graphics: u32,
    pub(super) compute: u32,
    pub(super) transfer: u32,
    pub(super) compute_queue_count: u32,
    pub(super) transfer_queue_count: u32,
}

impl QueueFamilies {
    pub(super) fn new(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        surface: &Surface,
    ) -> Result<QueueFamilies, InitError> {
        let queuefamilyproperties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut graphics: Option<u32> = None;
        let mut compute: Option<u32> = None;
        let mut transfer: Option<u32> = None;
        for (index, qfam) in queuefamilyproperties.iter().enumerate() {
            // TODO: Consider cases where the queue for dealing with a surface is different
            // from the queue that draws graphics
            if qfam.queue_count > 0
                && qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && surface.get_physical_device_surface_support(physical_device, index as u32)?
            {
                graphics = Some(index as u32);
            }
            if qfam.queue_count > 0
                && qfam.queue_flags.contains(vk::QueueFlags::COMPUTE)
                && (!qfam.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                && surface.get_physical_device_surface_support(physical_device, index as u32)?
            {
                compute = Some(index as u32);
            }
            if qfam.queue_count > 0
                && qfam.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && (!qfam
                    .queue_flags
                    .intersects(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE))
                && surface.get_physical_device_surface_support(physical_device, index as u32)?
            {
                transfer = Some(index as u32);
            }
        }
        if graphics.is_none() {
            return Err(InitError::DeviceSelectionError(
                "No valid queues exist for graphics!",
            ));
        }
        let graphics_index = graphics.unwrap();
        let compute_index = compute.unwrap_or(graphics_index);
        let transfer_index = transfer.unwrap_or(compute_index);
        Ok(QueueFamilies {
            graphics_queue_count: queuefamilyproperties[graphics_index as usize].queue_count,
            compute_queue_count: queuefamilyproperties[compute_index as usize].queue_count,
            transfer_queue_count: queuefamilyproperties[transfer_index as usize].queue_count,
            graphics: graphics_index,
            compute: compute_index,
            transfer: transfer_index,
        })
    }
}

pub(super) struct Queues {
    pub(super) graphics: vk::Queue,
    pub(super) transfer: vk::Queue,
    pub(super) compute: vk::Queue,
}
