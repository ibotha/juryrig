use std::mem::size_of;

use ash::{
    vk::{self, CommandBuffer},
    Device,
};
use gpu_allocator::vulkan::Allocator;

use super::{buffer::Buffer, error::VulkanError, VertexBufferBindings};

#[repr(C)]
pub struct ShaderVertexData {
    pub position: na::Vector3<f32>,
    pub uv: na::Vector2<f32>,
    pub normal: na::Vector3<f32>,
}

// A vulkan mesh that will not be changed during runtime.
pub struct StaticMesh {
    index_buffer: Buffer<u32>,
    vertex_buffer: Buffer<ShaderVertexData>,
}

impl StaticMesh {
    pub fn new(
        allocator: &mut Allocator,
        logical_device: &Device,
        index_data: &[u32],
        vertex_data: &[ShaderVertexData],
    ) -> Result<StaticMesh, VulkanError> {
        let mut index_buffer = Buffer::<u32>::new(
            allocator,
            logical_device,
            (index_data.len()) as u64,
            vk::BufferUsageFlags::INDEX_BUFFER,
            "index",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let mut vertex_buffer = Buffer::<ShaderVertexData>::new(
            allocator,
            logical_device,
            (vertex_data.len()) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        index_buffer.copy(index_data);
        vertex_buffer.copy(vertex_data);

        Ok(StaticMesh {
            index_buffer,
            vertex_buffer,
        })
    }

    pub fn bind(&self, logical_device: &Device, command_buffer: CommandBuffer) {
        unsafe {
            logical_device.cmd_bind_index_buffer(
                command_buffer,
                self.index_buffer.buffer,
                0,
                vk::IndexType::UINT32,
            );

            logical_device.cmd_bind_vertex_buffers(
                command_buffer,
                VertexBufferBindings::MeshBuffer as u32,
                &[self.vertex_buffer.buffer],
                &[0],
            );
        }
    }

    pub fn index_count(&self) -> u64 {
        self.index_buffer.len()
    }

    pub(crate) unsafe fn cleanup(
        &mut self,
        allocator: &mut std::mem::ManuallyDrop<Allocator>,
        logical_device: &Device,
    ) -> () {
        self.index_buffer.cleanup(allocator, logical_device);

        self.vertex_buffer.cleanup(allocator, logical_device);
    }
}
