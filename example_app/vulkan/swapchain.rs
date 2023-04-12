use ash::{
    extensions::khr,
    vk::{self, Framebuffer, PipelineStageFlags, Queue, SurfaceFormatKHR},
    Device, Instance,
};
use gpu_allocator::{vulkan::Allocator, MemoryLocation};

use super::{buffer::Image, surface::Surface, QueueFamilies};

pub(super) struct Swapchain {
    loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    image_views: Vec<vk::ImageView>,
    frame_buffers: Vec<vk::Framebuffer>,
    surface_format: vk::SurfaceFormatKHR,
    pub(super) extent: vk::Extent2D,
    image_available: Vec<vk::Semaphore>,
    rendering_finished: Vec<vk::Semaphore>,
    may_begin_drawing: Vec<vk::Fence>,
    amount_of_images: u32,
    current_image: usize,
    depth_image: Image,
    depth_imageview: vk::ImageView,
}

pub(super) struct FrameBufferInfo {
    pub(super) semaphores_available: [vk::Semaphore; 1],
    pub(super) semaphores_finished: [vk::Semaphore; 1],
    pub(super) may_begin_fence: vk::Fence,
    pub(super) waiting_stages: [PipelineStageFlags; 1],
    pub(super) image_index: u32,
    pub(super) framebuffer: Framebuffer,
    pub(super) queue: Queue,
}

impl Swapchain {
    pub(super) fn init(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        logical_device: &Device,
        allocator: &mut Allocator,
        surface: &Surface,
        queue_families: &QueueFamilies,
        surface_format: SurfaceFormatKHR, // HDR
                                          // VSYNC
                                          // Max-Framerate
    ) -> Result<Swapchain, vk::Result> {
        let surface_capabilities = surface.get_capabilities(physical_device)?;
        let extent = surface_capabilities.current_extent;
        let surface_present_modes = surface.get_present_modes(physical_device)?;

        let queuefamilies = [queue_families.graphics_q_index.unwrap()];

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface.surface)
            .min_image_count(
                3.max(surface_capabilities.min_image_count)
                    .min(surface_capabilities.max_image_count),
            )
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
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
        let amount_of_images = swapchain_images.len() as u32;
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
                .format(surface_format.format)
                .subresource_range(*subresource_range);

            let imageview =
                unsafe { logical_device.create_image_view(&imageview_create_info, None) }?;

            image_views.push(imageview);
        }

        let extent3d = vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        };

        let depth_image_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D32_SFLOAT)
            .extent(extent3d)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queuefamilies);

        let depth_image = Image::new(
            allocator,
            &logical_device,
            &depth_image_info,
            MemoryLocation::GpuOnly,
            "depth buffer",
            None,
        )?;

        let subresource_range = vk::ImageSubresourceRange::builder()
            .aspect_mask(vk::ImageAspectFlags::DEPTH)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);
        let imageview_create_info = vk::ImageViewCreateInfo::builder()
            .image(depth_image.image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::D32_SFLOAT)
            .subresource_range(*subresource_range);
        let depth_imageview =
            unsafe { logical_device.create_image_view(&imageview_create_info, None) }?;

        let mut image_available = vec![];
        let mut rendering_finished = vec![];
        let mut may_begin_drawing = vec![];
        let semaphoreinfo = vk::SemaphoreCreateInfo::builder();
        let fenceinfo = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
        for _ in 0..amount_of_images {
            let semaphore_available =
                unsafe { logical_device.create_semaphore(&semaphoreinfo, None) }?;
            let semaphore_finished =
                unsafe { logical_device.create_semaphore(&semaphoreinfo, None) }?;
            image_available.push(semaphore_available);
            rendering_finished.push(semaphore_finished);
            let fence = unsafe { logical_device.create_fence(&fenceinfo, None) }?;
            may_begin_drawing.push(fence);
        }

        Ok(Swapchain {
            loader: swapchain_loader,
            swapchain,
            image_views,
            extent,
            surface_format,
            frame_buffers: vec![],
            amount_of_images,
            image_available,
            may_begin_drawing,
            rendering_finished,
            current_image: 0,
            depth_image,
            depth_imageview,
        })
    }

    pub(super) fn create_framebuffers(
        &mut self,
        logical_device: &ash::Device,
        renderpass: vk::RenderPass,
    ) -> Result<(), vk::Result> {
        for iv in &self.image_views {
            let iview = [*iv, self.depth_imageview];
            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .render_pass(renderpass)
                .attachments(&iview)
                .width(self.extent.width)
                .height(self.extent.height)
                .layers(1);
            let fb = unsafe { logical_device.create_framebuffer(&framebuffer_info, None) }?;
            self.frame_buffers.push(fb);
        }
        Ok(())
    }

    pub(super) fn get_next_framebuffer(
        &mut self,
        logical_device: &Device,
        queue: Queue,
    ) -> Result<FrameBufferInfo, vk::Result> {
        // Select next image index
        self.current_image = (self.current_image + 1) % self.amount_of_images as usize;
        // Wait for image to be available
        let (image_index, _) = unsafe {
            self.loader
                .acquire_next_image(
                    self.swapchain,
                    std::u64::MAX,
                    self.image_available[self.current_image],
                    ash::vk::Fence::null(),
                )
                .expect("image acquisition trouble")
        };

        unsafe {
            logical_device
                .wait_for_fences(
                    &[self.may_begin_drawing[self.current_image]],
                    true,
                    std::u64::MAX,
                )
                .expect("fence-waiting");

            logical_device
                .reset_fences(&[self.may_begin_drawing[self.current_image]])
                .expect("resetting fences");
        }

        let semaphores_available = [self.image_available[self.current_image]];
        let waiting_stages = [ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let semaphores_finished = [self.rendering_finished[self.current_image]];

        Ok(FrameBufferInfo {
            semaphores_available,
            waiting_stages,
            semaphores_finished,
            framebuffer: self.frame_buffers[image_index as usize],
            image_index,
            may_begin_fence: self.may_begin_drawing[self.current_image],
            queue,
        })
    }

    pub(super) fn present_framebuffer(&mut self, frame_buffer_info: &FrameBufferInfo) {
        let swapchains = [self.swapchain];
        let indices = [frame_buffer_info.image_index as u32];
        let present_info = ash::vk::PresentInfoKHR::builder()
            .wait_semaphores(&frame_buffer_info.semaphores_finished)
            .swapchains(&swapchains)
            .image_indices(&indices);
        unsafe {
            self.loader
                .queue_present(frame_buffer_info.queue, &present_info)
                .expect("queue presentation");
        };
    }

    pub(super) unsafe fn cleanup(
        &mut self,
        logical_device: &Device,
        allocator: &mut Allocator,
    ) -> () {
        self.depth_image.cleanup(allocator, logical_device);
        logical_device.destroy_image_view(self.depth_imageview, None);

        for fence in &self.may_begin_drawing {
            logical_device.destroy_fence(*fence, None);
        }
        for semaphore in &self.image_available {
            logical_device.destroy_semaphore(*semaphore, None);
        }
        for semaphore in &self.rendering_finished {
            logical_device.destroy_semaphore(*semaphore, None);
        }
        for fb in &self.frame_buffers {
            logical_device.destroy_framebuffer(*fb, None);
        }
        for iv in &self.image_views {
            logical_device.destroy_image_view(*iv, None);
        }
        self.loader.destroy_swapchain(self.swapchain, None)
    }

    pub(crate) fn size(&self) -> usize {
        self.frame_buffers.len()
    }
}
