use std::collections::HashMap;

use ash::{
    vk::{self, CommandPool},
    Device,
};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};

use crate::jr_image::{HDRImage, RGBAImage};

use super::{
    buffer::Buffer,
    error::{InitError, RuntimeError},
    Pools, Queues,
};

use uuid::Uuid;

pub(super) struct Texture {
    pub(super) image: vk::Image,
    pub width: u32,
    pub height: u32,
    pub(super) image_view: vk::ImageView,
    pub(super) sampler: vk::Sampler,
    allocation: Option<Allocation>,
}

impl Texture {
    pub(super) fn new(
        allocator: &mut Allocator,
        logical_device: &Device,
        width: u32,
        height: u32,
        name: &str,
        queue_families: &[u32],
    ) -> Result<Texture, vk::Result> {
        let image_extent = vk::Extent3D {
            depth: 1,
            height: height,
            width: width,
        };
        let image_create_info = vk::ImageCreateInfo::builder()
            .extent(image_extent)
            .format(vk::Format::R8G8B8A8_SRGB)
            .image_type(vk::ImageType::TYPE_2D)
            .mip_levels(1)
            .tiling(vk::ImageTiling::LINEAR)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .array_layers(1)
            .queue_family_indices(queue_families)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .samples(vk::SampleCountFlags::TYPE_1);

        let image = unsafe { logical_device.create_image(&image_create_info, None)? };

        let requirements = unsafe { logical_device.get_image_memory_requirements(image) };
        let allocation = allocator
            .allocate(&AllocationCreateDesc {
                name: name,
                requirements,
                linear: false,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();

        // Bind memory to the buffer
        unsafe {
            logical_device.bind_image_memory(image, allocation.memory(), allocation.offset())?
        };

        let subresource_range = vk::ImageSubresourceRange::builder()
            .base_array_layer(0)
            .level_count(1)
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .layer_count(1)
            .build();

        let view_create_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .subresource_range(subresource_range);

        let image_view = unsafe { logical_device.create_image_view(&view_create_info, None) }?;

        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(16.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(0.0);

        let sampler = unsafe { logical_device.create_sampler(&sampler_info, None) }?;

        Ok(Texture {
            image,
            width,
            height,
            image_view,
            sampler,
            allocation: Some(allocation),
        })
    }

    pub(super) fn upload<T>(
        &mut self,
        allocator: &mut Allocator,
        logical_device: &Device,
        raw: &[T],
        queue: vk::Queue,
        pool: CommandPool,
    ) -> Result<(), vk::Result> {
        let mut buffer = Buffer::new(
            allocator,
            &logical_device,
            raw.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            "Image Temp",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        match buffer.copy(&raw) {
            Err(_) => panic!("Could not upload texture!"),
            Ok(_) => {}
        }

        let commandbuf_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(pool)
            .command_buffer_count(1);
        let copycmdbuffer =
            unsafe { logical_device.allocate_command_buffers(&commandbuf_allocate_info) }.unwrap()
                [0];

        let cmdbegininfo = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { logical_device.begin_command_buffer(copycmdbuffer, &cmdbegininfo) }?;

        let barrier = vk::ImageMemoryBarrier::builder()
            .image(self.image)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .build();

        unsafe {
            logical_device.cmd_pipeline_barrier(
                copycmdbuffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            )
        };
        let image_subresource = vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        };
        let region = vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            image_extent: vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            },
            image_subresource,
            ..Default::default()
        };
        unsafe {
            logical_device.cmd_copy_buffer_to_image(
                copycmdbuffer,
                buffer.buffer,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        let barrier = vk::ImageMemoryBarrier::builder()
            .image(self.image)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .build();
        unsafe {
            logical_device.cmd_pipeline_barrier(
                copycmdbuffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            )
        };

        unsafe { logical_device.end_command_buffer(copycmdbuffer) }?;
        let submit_infos = [vk::SubmitInfo::builder()
            .command_buffers(&[copycmdbuffer])
            .build()];
        let fence = unsafe { logical_device.create_fence(&vk::FenceCreateInfo::default(), None) }?;
        unsafe { logical_device.queue_submit(queue, &submit_infos, fence) }?;
        unsafe { logical_device.wait_for_fences(&[fence], true, std::u64::MAX) }?;
        unsafe { logical_device.destroy_fence(fence, None) };
        unsafe { buffer.cleanup(allocator, &logical_device) };
        unsafe { logical_device.free_command_buffers(pool, &[copycmdbuffer]) };
        Ok(())
    }

    pub(super) unsafe fn cleanup(&mut self, allocator: &mut Allocator, logical_device: &Device) {
        logical_device.destroy_sampler(self.sampler, None);

        logical_device.destroy_image_view(self.image_view, None);

        logical_device.destroy_image(self.image, None);

        allocator.free(self.allocation.take().unwrap()).unwrap();
    }
}

pub struct TextureHandle {
    id: Uuid,
}

pub(super) struct TextureStore {
    textures_map: HashMap<Uuid, u32>,
    pub textures: Vec<Texture>,
}

impl TextureStore {
    pub(super) fn new() -> Result<TextureStore, InitError> {
        Ok(TextureStore {
            textures_map: HashMap::new(),
            textures: vec![],
        })
    }

    // Allocates and registers an empty image
    pub(super) fn create_empty_texture(&mut self, width: u32, height: u32) {
        todo!()
    }

    // Allocates and registers an empty image
    pub(super) fn register_texture(
        &mut self,
        allocator: &mut Allocator,
        logical_device: &Device,
        image: &RGBAImage,
        queues: &[u32],
        transfer_queue: vk::Queue,
        transfer_cmd_pool: vk::CommandPool,
    ) -> Result<TextureHandle, RuntimeError> {
        static mut a: u32 = 0;
        unsafe { a = a + 1 };
        let id = Uuid::new_v4();
        let mut texture = Texture::new(
            allocator,
            logical_device,
            image.width,
            image.height,
            format!("t-{}", &id).as_str(),
            queues,
        )?;
        texture.upload(
            allocator,
            logical_device,
            &image.data,
            transfer_queue,
            transfer_cmd_pool,
        )?;
        self.textures.push(texture);
        self.textures_map
            .insert(id, (self.textures.len() - 1) as u32);
        Ok(TextureHandle { id })
    }

    // Allocates and registers an empty image
    pub(super) fn register_hdr_texture(&mut self, image: HDRImage) {
        todo!()
    }

    pub(super) fn cleanup(&mut self, allocator: &mut Allocator, logical_device: &Device) {
        for t in &mut self.textures {
            unsafe {
                t.cleanup(allocator, logical_device);
            }
        }
    }

    pub(crate) fn get_descriptor_image_info(&self) -> Vec<vk::DescriptorImageInfo> {
        self.textures
            .iter()
            .map(|texture| {
                vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture.image_view)
                    .sampler(texture.sampler)
                    .build()
            })
            .collect()
    }
}
