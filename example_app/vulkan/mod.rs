mod buffer;
mod camera;
mod debug;
mod initialisation;
mod mesh;
mod pipeline;
mod surface;
mod swapchain;
mod texture;

use crate::jr_image::RGBAImage;

use self::{
    error::{InitError, RuntimeError},
    initialisation::{
        create_instance, init_device_and_queues, init_physical_device_and_properties,
        QueueFamilies, Queues,
    },
    mesh::ShaderVertexData,
    surface::Surface,
    texture::{TextureHandle, TextureStore},
};
use ash::{
    vk::{self, DescriptorImageInfo},
    Device, Entry, Instance,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use log::info;
use na::{Vector2, Vector3};
use winit::window::Window;

use self::buffer::Buffer;
use self::camera::Camera;
use self::debug::Debug;
use self::mesh::StaticMesh;
use self::pipeline::Pipeline;
use self::swapchain::Swapchain;
use self::texture::Texture;

mod error;

#[derive(Copy, Clone)]
enum VertexBufferBindings {
    InstanceBuffer = 0,
    MeshBuffer = 1,
}

#[repr(C)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],
    pub texture_index: u32,
}

struct Pools {
    graphics: vk::CommandPool,
    compute: vk::CommandPool,
    transfer: vk::CommandPool,
}

impl Pools {
    fn init(
        logical_device: &ash::Device,
        queue_families: &QueueFamilies,
    ) -> Result<Pools, vk::Result> {
        // Graphics Pool
        let graphics_commandpool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_families.graphics)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandpool_graphics =
            unsafe { logical_device.create_command_pool(&graphics_commandpool_info, None) }?;

        // Compute Pool
        let compute_commandpool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_families.compute)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandpool_compute =
            unsafe { logical_device.create_command_pool(&graphics_commandpool_info, None) }?;

        // Transfer Pool
        let transfer_commandpool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_families.transfer)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandpool_transfer =
            unsafe { logical_device.create_command_pool(&transfer_commandpool_info, None) }?;

        Ok(Pools {
            graphics: commandpool_graphics,
            compute: commandpool_compute,
            transfer: commandpool_transfer,
        })
    }
    fn cleanup(&self, logical_device: &ash::Device) {
        unsafe {
            logical_device.destroy_command_pool(self.graphics, None);
            logical_device.destroy_command_pool(self.compute, None);
            logical_device.destroy_command_pool(self.transfer, None);
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
    instance_buffer: Buffer<InstanceData>,
    pub camera: Camera,
    cube: StaticMesh,
    texture_store: TextureStore,
}

impl Vulkan {
    pub fn new(window: &Window) -> std::result::Result<Self, InitError> {
        let entry = unsafe { Entry::load() }?;

        let mut debug_create_info = Debug::create_info();

        let instance = create_instance(&entry, &window, &mut debug_create_info)?;

        // Vulkan debugging
        let debug = Debug::new(&entry, &instance, debug_create_info)?;

        let surface: Surface = Surface::new(&window, &entry, &instance)?;

        let (physical_device, physical_device_properties) =
            init_physical_device_and_properties(&instance)?;

        let queue_families = QueueFamilies::new(&instance, physical_device, &surface)?;

        let (logical_device, queues) =
            init_device_and_queues(&instance, physical_device, &queue_families)?;
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

        let instance_buffer = Buffer::<InstanceData>::new(
            &mut allocator,
            &logical_device,
            4,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        let mut my_camera = Camera::default();
        my_camera.move_backward(6f32);

        let index_data: Vec<u32> = vec![
            0, 1, 2, 3, 4, 5, // front face
            6, 7, 8, 9, 10, 11, // top face
            12, 13, 14, 15, 16, 17, // bottom face
            18, 19, 20, 21, 22, 23, // back face
            24, 25, 26, 27, 28, 29, // left face
            30, 31, 32, 33, 34, 35, // right face
        ];

        let vertex_data = [
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.374988, 0.666810),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.343755, 0.733482),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.343804, 0.666761),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.312471, 0.733295),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.281288, 0.666747),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.312494, 0.666682),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.406424, 0.733344),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.375017, 0.666931),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.406197, 0.667131),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.374875, 0.667210),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.343703, 0.733315),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.343703, 0.667158),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.343723, 0.733344),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.312531, 0.666785),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.343706, 0.666848),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.406446, 0.733250),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.375170, 0.667212),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.406162, 0.666986),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.374988, 0.666810),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.375027, 0.733414),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.343755, 0.733482),
                normal: Vector3::new(-0.0000, 1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.312471, 0.733295),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.281267, 0.733332),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.281288, 0.666747),
                normal: Vector3::new(-1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.406424, 0.733344),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.375060, 0.733192),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.375017, 0.666931),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.374875, 0.667210),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.374875, 0.733262),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.343703, 0.733315),
                normal: Vector3::new(-0.0000, -1.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.343723, 0.733344),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, 1.000000),
                uv: Vector2::new(0.312471, 0.733295),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, 1.000000),
                uv: Vector2::new(0.312531, 0.666785),
                normal: Vector3::new(1.0000, -0.0000, -0.0000),
            },
            ShaderVertexData {
                position: Vector3::new(-1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.406446, 0.733250),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, 1.000000, -1.000000),
                uv: Vector2::new(0.375164, 0.733269),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
            ShaderVertexData {
                position: Vector3::new(1.000000, -1.000000, -1.000000),
                uv: Vector2::new(0.375170, 0.667212),
                normal: Vector3::new(-0.0000, -0.0000, -1.0000),
            },
        ];

        let cube = StaticMesh::new(&mut allocator, &logical_device, &index_data, &vertex_data)?;
        let texture_store = TextureStore::new()?;
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
            instance_buffer,
            cube,
            camera: my_camera,
            texture_store,
        })
    }

    pub fn register_texture(&mut self, image: &RGBAImage) -> Result<TextureHandle, RuntimeError> {
        self.texture_store.register_texture(
            &mut self.allocator,
            &self.logical_device,
            &image,
            &[self.queue_families.graphics],
            self.queues.graphics,
            self.command_buffer_pools.graphics,
        )
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
            .command_pool(pools.graphics)
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
            static mut stat: f32 = 0f32;
            unsafe { stat = stat + 0.001f32 };
            let a = unsafe { stat };

            let instance_data = [
                InstanceData {
                    model: (na::Matrix4::new_translation(&na::Vector3::new(0f32, 0f32, 0f32))
                        * na::Matrix4::from_euler_angles(a, 0f32, 0f32))
                    .into(),
                    texture_index: 0,
                },
                InstanceData {
                    model: (na::Matrix4::new_translation(&na::Vector3::new(0f32, 0f32, 3f32))
                        * na::Matrix4::from_euler_angles(0f32, a / 3f32, 0f32))
                    .into(),
                    texture_index: 0,
                },
                InstanceData {
                    model: (na::Matrix4::new_translation(&na::Vector3::new(0f32, 3f32, 0f32))
                        * na::Matrix4::from_euler_angles(0f32, 0f32, a / 2.5f32))
                    .into(),
                    texture_index: 1,
                },
                InstanceData {
                    model: (na::Matrix4::new_translation(&na::Vector3::new(3f32, 0f32, 0f32))
                        * na::Matrix4::from_euler_angles(0f32, a / 2f32, a / 3f32))
                    .into(),
                    texture_index: 1,
                },
            ];

            self.instance_buffer
                .copy(&instance_data)
                .expect("Couldn't copy!!!!");

            let renderpass_begininfo = vk::RenderPassBeginInfo::builder()
                .render_pass(self.renderpass)
                .framebuffer(frame_buffer_info.framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain.extent,
                })
                .clear_values(&clearvalues);

            let descriptor_image_infos = self.texture_store.get_descriptor_image_info();
            let descriptor_writes = [vk::WriteDescriptorSet {
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                dst_set: self.graphics_pipeline.descriptor_sets
                    [frame_buffer_info.image_index as usize],
                dst_binding: 0,
                dst_array_element: 0,
                p_image_info: descriptor_image_infos.as_ptr(),
                descriptor_count: 2,
                ..Default::default()
            }];
            unsafe {
                self.logical_device
                    .update_descriptor_sets(&descriptor_writes, &[]);

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

                let projection: [[f32; 4]; 4] =
                    (self.camera.projectionmatrix * self.camera.viewmatrix).into();
                self.logical_device.cmd_push_constants(
                    commandbuffer,
                    self.graphics_pipeline.layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    &std::mem::transmute::<[[f32; 4]; 4], [u8; 64]>(projection),
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
                self.cube.bind(&self.logical_device, commandbuffer);

                self.logical_device.cmd_bind_vertex_buffers(
                    commandbuffer,
                    VertexBufferBindings::InstanceBuffer as u32,
                    &[self.instance_buffer.buffer],
                    &[0],
                );

                self.logical_device.cmd_draw_indexed(
                    commandbuffer,
                    self.cube.index_count() as u32,
                    2,
                    0,
                    0,
                    0,
                );

                self.logical_device.cmd_draw_indexed(
                    commandbuffer,
                    self.cube.index_count() as u32,
                    2,
                    0,
                    0,
                    2,
                );

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

            self.instance_buffer
                .cleanup(&mut self.allocator, &self.logical_device);

            self.texture_store
                .cleanup(&mut self.allocator, &self.logical_device);

            self.cube.cleanup(&mut self.allocator, &self.logical_device);

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
