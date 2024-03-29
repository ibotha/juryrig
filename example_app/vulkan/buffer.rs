use std::{marker::PhantomData, mem::size_of};

use ash::{vk, Device};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};

pub(super) struct Buffer<T> {
    pub(super) buffer: vk::Buffer,
    allocation: Option<Allocation>,
    phantom: PhantomData<T>,
    size: u64,
}

impl<T> Buffer<T> {
    pub(super) fn new(
        allocator: &mut Allocator,
        logical_device: &Device,
        size: u64,
        usage: vk::BufferUsageFlags,
        name: &str,
        mem_location: gpu_allocator::MemoryLocation,
    ) -> Result<Buffer<T>, ash::vk::Result> {
        let buffer_create_info = vk::BufferCreateInfo::builder()
            .size((size as usize * size_of::<T>()) as u64)
            .usage(usage);

        let buffer = unsafe {
            logical_device
                .create_buffer(&buffer_create_info, None)
                .unwrap()
        };

        let requirements = unsafe { logical_device.get_buffer_memory_requirements(buffer) };
        let allocation = allocator
            .allocate(&AllocationCreateDesc {
                name: name,
                requirements,
                linear: true,
                location: mem_location,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();

        // Bind memory to the buffer
        unsafe {
            logical_device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?
        };
        Ok(Buffer {
            buffer,
            allocation: Some(allocation),
            size,
            phantom: PhantomData,
        })
    }

    pub(super) fn copy(&mut self, in_data: &[T]) -> Result<(), ()> {
        match &self.allocation {
            Some(allocation) => {
                let data_ptr = allocation.mapped_ptr().unwrap().cast().as_ptr();
                unsafe {
                    std::ptr::copy_nonoverlapping(in_data.as_ptr(), data_ptr, in_data.len());
                }
                Ok(())
            }
            None => Err(()),
        }
    }

    pub(super) unsafe fn cleanup(&mut self, allocator: &mut Allocator, logical_device: &Device) {
        logical_device.destroy_buffer(self.buffer, None);

        allocator.free(self.allocation.take().unwrap()).unwrap();
    }

    pub fn len(&self) -> u64 {
        self.size
    }
}

pub(super) struct Image {
    pub(super) image: vk::Image,
    allocation: Option<Allocation>,
}

impl Image {
    pub(super) fn new(
        allocator: &mut Allocator,
        logical_device: &Device,
        create_info: &vk::ImageCreateInfo,
        location: gpu_allocator::MemoryLocation,
        name: &str,
        linear: Option<bool>,
    ) -> Result<Image, ash::vk::Result> {
        let image = unsafe { logical_device.create_image(&create_info, None)? };

        let requirements = unsafe { logical_device.get_image_memory_requirements(image) };
        let allocation = allocator
            .allocate(&AllocationCreateDesc {
                name: name,
                requirements,
                linear: linear.unwrap_or(false),
                location,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();

        // Bind memory to the buffer
        unsafe {
            logical_device.bind_image_memory(image, allocation.memory(), allocation.offset())?
        };

        Ok(Image {
            image,
            allocation: Some(allocation),
        })
    }

    pub(super) fn copy<T>(&mut self, in_data: &Vec<T>) -> Result<(), ()> {
        match &self.allocation {
            Some(allocation) => {
                let data_ptr = allocation.mapped_ptr().unwrap().cast().as_ptr();
                unsafe {
                    std::ptr::copy_nonoverlapping(in_data.as_ptr(), data_ptr, in_data.len());
                }
                Ok(())
            }
            None => Err(()),
        }
    }

    pub(super) unsafe fn cleanup(&mut self, allocator: &mut Allocator, logical_device: &Device) {
        logical_device.destroy_image(self.image, None);

        allocator.free(self.allocation.take().unwrap()).unwrap();
    }
}
