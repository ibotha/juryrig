use std::{marker::PhantomData, mem::size_of};

use ash::{vk, Device};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use log::info;

pub(super) struct Buffer<T> {
    pub(super) buffer: vk::Buffer,
    allocation: Option<Allocation>,
    phantom: PhantomData<T>,
}

impl<T> Buffer<T> {
    pub(super) fn new(
        allocator: &mut Allocator,
        logical_device: &Device,
        size: u64,
        usage: vk::BufferUsageFlags,
        name: &str,
    ) -> Result<Buffer<T>, ash::vk::Result> {
        let buffer_create_info = vk::BufferCreateInfo::builder()
            .size(size * (size_of::<T>() as u64))
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
                location: gpu_allocator::MemoryLocation::CpuToGpu,
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
            phantom: PhantomData,
        })
    }

    pub(super) fn copy(&mut self, in_data: &Vec<T>) -> Result<(), ()> {
        match &self.allocation {
            Some(allocation) => {
                let data_ptr = allocation.mapped_ptr().unwrap().cast().as_ptr();
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        in_data.as_ptr(),
                        data_ptr,
                        in_data.len() * size_of::<T>(),
                    );
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
}
