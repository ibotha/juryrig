use ash::{
    extensions::{ext::DebugUtils, khr},
    vk::{self},
    Device, Entry, Instance,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use log::info;
use std::ffi::{CStr, CString};
use winit::window::Window;

mod surface;
use self::{buffer::Image, surface::Surface};

mod swapchain;
use self::swapchain::Swapchain;

mod pipeline;
use self::pipeline::Pipeline;

mod debug;
use self::debug::Debug;

mod buffer;
use self::buffer::Buffer;

mod texture;
use self::texture::Texture;

mod camera;
use self::camera::Camera;

struct QueueFamilies {
    graphics_q_index: Option<u32>,
    transfer_q_index: Option<u32>,
}

impl QueueFamilies {
    fn new(
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
    graphics: vk::Queue,
    transfer: vk::Queue,
}

struct Pools {
    commandpool_graphics: vk::CommandPool,
    commandpool_transfer: vk::CommandPool,
}

impl Pools {
    fn init(
        logical_device: &ash::Device,
        queue_families: &QueueFamilies,
    ) -> Result<Pools, vk::Result> {
        let graphics_commandpool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_families.graphics_q_index.unwrap())
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandpool_graphics =
            unsafe { logical_device.create_command_pool(&graphics_commandpool_info, None) }?;
        let transfer_commandpool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_families.transfer_q_index.unwrap())
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandpool_transfer =
            unsafe { logical_device.create_command_pool(&transfer_commandpool_info, None) }?;

        Ok(Pools {
            commandpool_graphics,
            commandpool_transfer,
        })
    }
    fn cleanup(&self, logical_device: &ash::Device) {
        unsafe {
            logical_device.destroy_command_pool(self.commandpool_graphics, None);
            logical_device.destroy_command_pool(self.commandpool_transfer, None);
        }
    }
}

pub struct Vulkan {
    instance: Instance,
    entry: Entry,
    debug: std::mem::ManuallyDrop<Debug>,
    surface: std::mem::ManuallyDrop<Surface>,
    queue_families: QueueFamilies,
    logical_device: Device,
    queues: Queues,
    swapchain: Swapchain,
    renderpass: vk::RenderPass,
    graphics_pipeline: Pipeline,
    command_buffer_pools: Pools,
    command_buffers: Vec<vk::CommandBuffer>,
    allocator: std::mem::ManuallyDrop<Allocator>,
    index_buffer: Buffer<u32>,
    vertex_buffer: Buffer<f32>,
    instance_buffer: Buffer<[[f32; 4]; 4]>,
    atlas: Texture,
    pub camera: Camera,
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

        extension_name_pointers.push(Surface::extention_name_ptr());

        return extension_name_pointers;
    }

    pub fn new(window: &Window) -> std::result::Result<Self, Box<dyn std::error::Error>> {
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
        let surface_format = surface
            .get_formats(physical_device)?
            .first()
            .unwrap()
            .clone();

        let mut allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: logical_device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: true,
        })?;

        let mut swapchain = Swapchain::init(
            &instance,
            physical_device,
            &logical_device,
            &mut allocator,
            &surface,
            &queue_families,
            surface_format,
        )?;

        let renderpass = Self::init_renderpass(&logical_device, surface_format)?;

        swapchain.create_framebuffers(&logical_device, renderpass)?;

        let graphics_pipeline = Pipeline::init(&logical_device, &swapchain, &renderpass)?;

        let pools = Pools::init(&logical_device, &queue_families)?;

        let command_buffers =
            Self::create_commandbuffers(&logical_device, &pools, swapchain.size())?;

        let index_buffer = Buffer::<u32>::new(
            &mut allocator,
            &logical_device,
            12 * 3,
            vk::BufferUsageFlags::INDEX_BUFFER,
            "index",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let vertex_buffer = Buffer::<f32>::new(
            &mut allocator,
            &logical_device,
            (3 + 2 + 3) * 36,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let instance_buffer = Buffer::<[[f32; 4]; 4]>::new(
            &mut allocator,
            &logical_device,
            4,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        let mut image = image::io::Reader::open("MC_Atlas.png")?.decode()?.flipv();

        let queuefamilies = [queue_families.graphics_q_index.unwrap()];

        let mut atlas = Texture::new(
            &mut allocator,
            &logical_device,
            image.width(),
            image.height(),
            "atlas",
            &queuefamilies,
        )?;

        let raw = image.as_mut_rgba8().unwrap().as_raw();

        atlas.upload(&mut allocator, &logical_device, &raw, &queues, &pools);

        let descriptor_image_infos = [vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(atlas.image_view)
            .sampler(atlas.sampler)
            .build()];

        for i in 0..3 {
            let descriptor_writes = [vk::WriteDescriptorSet::builder()
                .dst_set(graphics_pipeline.descriptor_sets[i])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .dst_array_element(0)
                .image_info(&descriptor_image_infos)
                .build()];
            unsafe { logical_device.update_descriptor_sets(&descriptor_writes, &[]) };
        }

        let mut my_camera = Camera::default();
        my_camera.move_backward(6f32);

        Ok(Self {
            instance,
            entry,
            debug: std::mem::ManuallyDrop::new(debug),
            surface: std::mem::ManuallyDrop::new(surface),
            queue_families,
            logical_device,
            queues,
            swapchain,
            renderpass,
            graphics_pipeline,
            command_buffer_pools: pools,
            command_buffers,
            allocator: std::mem::ManuallyDrop::new(allocator),
            index_buffer,
            vertex_buffer,
            instance_buffer,
            atlas,
            camera: my_camera,
        })
    }

    fn create_instance(
        entry: &Entry,
        window: &Window,
        debug_create_info: &mut vk::DebugUtilsMessengerCreateInfoEXTBuilder,
    ) -> std::result::Result<Instance, vk::Result> {
        let engine_name: CString = CString::new("Juryrig").unwrap();
        let app_name: CString = CString::new(window.title().to_owned()).unwrap();

        // Layers and extentions

        let layer_name_pointers = Self::layer_name_pointers();
        let extension_name_pointers = Self::extension_name_pointers();

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

    fn init_device_and_queues(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        queue_families: &QueueFamilies,
    ) -> Result<(Device, Queues), vk::Result> {
        let layer_name_pointers = Vulkan::layer_name_pointers();

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

    fn init_renderpass(
        logical_device: &ash::Device,
        format: vk::SurfaceFormatKHR,
    ) -> Result<vk::RenderPass, vk::Result> {
        let attachments = [
            vk::AttachmentDescription::builder()
                .format(format.format)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .samples(vk::SampleCountFlags::TYPE_1)
                .build(),
            vk::AttachmentDescription::builder()
                .format(vk::Format::D32_SFLOAT)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .samples(vk::SampleCountFlags::TYPE_1)
                .build(),
        ];
        let color_attachment_references = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth_attachment_reference = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        let subpasses = [vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_references)
            .depth_stencil_attachment(&depth_attachment_reference)
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .build()];
        let subpass_dependencies = [vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_subpass(0)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .build()];
        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&subpass_dependencies);
        let renderpass = unsafe { logical_device.create_render_pass(&renderpass_info, None)? };
        Ok(renderpass)
    }

    fn create_commandbuffers(
        logical_device: &ash::Device,
        pools: &Pools,
        amount: usize,
    ) -> Result<Vec<vk::CommandBuffer>, vk::Result> {
        let commandbuf_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(pools.commandpool_graphics)
            .command_buffer_count(amount as u32);
        unsafe { logical_device.allocate_command_buffers(&commandbuf_allocate_info) }
    }

    pub(crate) fn swap_framebuffers(&mut self) -> Result<(), vk::Result> {
        let frame_buffer_info = self
            .swapchain
            .get_next_framebuffer(&self.logical_device, self.queues.graphics)?;

        // Runder commands
        {
            let commandbuffer_begininfo = vk::CommandBufferBeginInfo::builder();
            let commandbuffer = self.command_buffers[frame_buffer_info.image_index as usize];
            unsafe {
                self.logical_device
                    .begin_command_buffer(commandbuffer, &commandbuffer_begininfo)?;
            }
            let clearvalues = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.1, 0.1, 0.1, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let renderpass_begininfo = vk::RenderPassBeginInfo::builder()
                .render_pass(self.renderpass)
                .framebuffer(frame_buffer_info.framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain.extent,
                })
                .clear_values(&clearvalues);
            unsafe {
                self.logical_device.cmd_begin_render_pass(
                    commandbuffer,
                    &renderpass_begininfo,
                    vk::SubpassContents::INLINE,
                );
                self.logical_device.cmd_bind_pipeline(
                    commandbuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline.pipeline,
                );

                let index_data: Vec<u32> = vec![
                    0, 1, 2, 3, 4, 5, // front face
                    6, 7, 8, 9, 10, 11, // top face
                    12, 13, 14, 15, 16, 17, // bottom face
                    18, 19, 20, 21, 22, 23, // back face
                    24, 25, 26, 27, 28, 29, // left face
                    30, 31, 32, 33, 34, 35, // right face
                ];

                let vertex_data: Vec<f32> = vec![
                    -1.000000, 1.000000, -1.000000, // 0
                    0.374988, 0.666810, //
                    -0.0000, 1.0000, -0.0000, //
                    1.000000, 1.000000, 1.000000, // 1
                    0.343755, 0.733482, //
                    -0.0000, 1.0000, -0.0000, //
                    1.000000, 1.000000, -1.000000, // 2
                    0.343804, 0.666761, //
                    -0.0000, 1.0000, -0.0000, //
                    1.000000, 1.000000, 1.000000, // 3
                    0.312471, 0.733295, //
                    -1.0000, -0.0000, -0.0000, //
                    -1.000000, -1.000000, 1.000000, // 4
                    0.281288, 0.666747, //
                    -1.0000, -0.0000, -0.0000, //
                    1.000000, -1.000000, 1.000000, // 5
                    0.312494, 0.666682, //
                    -1.0000, -0.0000, -0.0000, //
                    -1.000000, 1.000000, 1.000000, // 6
                    0.406424, 0.733344, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, -1.000000, // 7
                    0.375017, 0.666931, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, 1.000000, // 8
                    0.406197, 0.667131, //
                    -0.0000, -1.0000, -0.0000, //
                    1.000000, -1.000000, -1.000000, // 9
                    0.374875, 0.667210, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, 1.000000, // 10
                    0.343703, 0.733315, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, -1.000000, // 11
                    0.343703, 0.667158, //
                    -0.0000, -1.0000, -0.0000, //
                    1.000000, 1.000000, -1.000000, // 12
                    0.343723, 0.733344, //
                    1.0000, -0.0000, -0.0000, //
                    1.000000, -1.000000, 1.000000, // 13
                    0.312531, 0.666785, //
                    1.0000, -0.0000, -0.0000, //
                    1.000000, -1.000000, -1.000000, // 14
                    0.343706, 0.666848, //
                    1.0000, -0.0000, -0.0000, //
                    -1.000000, 1.000000, -1.000000, // 15
                    0.406446, 0.733250, //
                    -0.0000, -0.0000, -1.0000, //
                    1.000000, -1.000000, -1.000000, // 16
                    0.375170, 0.667212, //
                    -0.0000, -0.0000, -1.0000, //
                    -1.000000, -1.000000, -1.000000, // 17
                    0.406162, 0.666986, //
                    -0.0000, -0.0000, -1.0000, //
                    -1.000000, 1.000000, -1.000000, // 18
                    0.374988, 0.666810, //
                    -0.0000, 1.0000, -0.0000, //
                    -1.000000, 1.000000, 1.000000, // 19
                    0.375027, 0.733414, //
                    -0.0000, 1.0000, -0.0000, //
                    1.000000, 1.000000, 1.000000, // 20
                    0.343755, 0.733482, //
                    -0.0000, 1.0000, -0.0000, //
                    1.000000, 1.000000, 1.000000, // 21
                    0.312471, 0.733295, //
                    -1.0000, -0.0000, -0.0000, //
                    -1.000000, 1.000000, 1.000000, // 22
                    0.281267, 0.733332, //
                    -1.0000, -0.0000, -0.0000, //
                    -1.000000, -1.000000, 1.000000, // 23
                    0.281288, 0.666747, //
                    -1.0000, -0.0000, -0.0000, //
                    -1.000000, 1.000000, 1.000000, // 24
                    0.406424, 0.733344, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, 1.000000, -1.000000, // 25
                    0.375060, 0.733192, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, -1.000000, // 26
                    0.375017, 0.666931, //
                    -0.0000, -1.0000, -0.0000, //
                    1.000000, -1.000000, -1.000000, // 27
                    0.374875, 0.667210, //
                    -0.0000, -1.0000, -0.0000, //
                    1.000000, -1.000000, 1.000000, // 28
                    0.374875, 0.733262, //
                    -0.0000, -1.0000, -0.0000, //
                    -1.000000, -1.000000, 1.000000, // 29
                    0.343703, 0.733315, //
                    -0.0000, -1.0000, -0.0000, //
                    1.000000, 1.000000, -1.000000, // 30
                    0.343723, 0.733344, //
                    1.0000, -0.0000, -0.0000, //
                    1.000000, 1.000000, 1.000000, // 31
                    0.312471, 0.733295, //
                    1.0000, -0.0000, -0.0000, //
                    1.000000, -1.000000, 1.000000, // 32
                    0.312531, 0.666785, //
                    1.0000, -0.0000, -0.0000, //
                    -1.000000, 1.000000, -1.000000, // 33
                    0.406446, 0.733250, //
                    -0.0000, -0.0000, -1.0000, //
                    1.000000, 1.000000, -1.000000, // 34
                    0.375164, 0.733269, //
                    -0.0000, -0.0000, -1.0000, //
                    1.000000, -1.000000, -1.000000, // 35
                    0.375170, 0.667212, //
                    -0.0000, -0.0000, -1.0000, //
                ];

                static mut a: f32 = 0.01f32;
                a += 0.01f32;

                let instance_data: Vec<[[f32; 4]; 4]> = vec![
                    (na::Matrix4::new_translation(&na::Vector3::new(0f32, 0f32, 0f32))
                        * na::Matrix4::from_euler_angles(a, 0f32, 0f32))
                    .into(),
                    (na::Matrix4::new_translation(&na::Vector3::new(0f32, 0f32, 3f32))
                        * na::Matrix4::from_euler_angles(0f32, a / 3f32, 0f32))
                    .into(),
                    (na::Matrix4::new_translation(&na::Vector3::new(0f32, 3f32, 0f32))
                        * na::Matrix4::from_euler_angles(0f32, 0f32, a / 2.5f32))
                    .into(),
                    (na::Matrix4::new_translation(&na::Vector3::new(3f32, 0f32, 0f32))
                        * na::Matrix4::from_euler_angles(0f32, a / 2f32, a / 3f32))
                    .into(),
                ];

                let projection: [[f32; 4]; 4] =
                    (self.camera.projectionmatrix * self.camera.viewmatrix).into();

                self.index_buffer
                    .copy(&index_data)
                    .expect("Couldn't copy!!!!");

                self.vertex_buffer
                    .copy(&vertex_data)
                    .expect("Couldn't copy!!!!");

                self.instance_buffer
                    .copy(&instance_data)
                    .expect("Couldn't copy!!!!");

                self.logical_device.cmd_push_constants(
                    commandbuffer,
                    self.graphics_pipeline.layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    &unsafe { std::mem::transmute::<[[f32; 4]; 4], [u8; 64]>(projection) },
                );

                self.logical_device.cmd_bind_descriptor_sets(
                    commandbuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline.layout,
                    0,
                    &[self.graphics_pipeline.descriptor_sets
                        [frame_buffer_info.image_index as usize]],
                    &[],
                );

                self.logical_device.cmd_bind_index_buffer(
                    commandbuffer,
                    self.index_buffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );

                self.logical_device.cmd_bind_vertex_buffers(
                    commandbuffer,
                    0,
                    &[self.instance_buffer.buffer, self.vertex_buffer.buffer],
                    &[0, 0],
                );

                self.logical_device
                    .cmd_draw_indexed(commandbuffer, 12 * 3, 4, 0, 0, 0);

                self.logical_device.cmd_end_render_pass(commandbuffer);
                self.logical_device.end_command_buffer(commandbuffer)?;
            }
        }

        let command_buffers = [self.command_buffers[frame_buffer_info.image_index as usize]];

        let submit_info = [ash::vk::SubmitInfo::builder()
            .wait_semaphores(&frame_buffer_info.semaphores_available)
            .wait_dst_stage_mask(&frame_buffer_info.waiting_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&frame_buffer_info.semaphores_finished)
            .build()];
        unsafe {
            self.logical_device
                .queue_submit(
                    frame_buffer_info.queue,
                    &submit_info,
                    frame_buffer_info.may_begin_fence,
                )
                .expect("queue submission");
        };

        self.swapchain.present_framebuffer(&frame_buffer_info);
        Ok(())
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        info!("Destroying vulkan");
        unsafe {
            self.logical_device
                .device_wait_idle()
                .expect("something wrong while waiting");

            self.atlas
                .cleanup(&mut self.allocator, &self.logical_device);

            self.instance_buffer
                .cleanup(&mut self.allocator, &self.logical_device);

            self.index_buffer
                .cleanup(&mut self.allocator, &self.logical_device);

            self.vertex_buffer
                .cleanup(&mut self.allocator, &self.logical_device);

            self.command_buffer_pools.cleanup(&self.logical_device);

            self.graphics_pipeline.cleanup(&self.logical_device);
            self.logical_device
                .destroy_render_pass(self.renderpass, None);
            self.swapchain
                .cleanup(&self.logical_device, &mut self.allocator);
            std::mem::ManuallyDrop::drop(&mut self.allocator);

            self.logical_device.destroy_device(None);
            std::mem::ManuallyDrop::drop(&mut self.surface);
            std::mem::ManuallyDrop::drop(&mut self.debug);
            self.instance.destroy_instance(None);
        }
    }
}
