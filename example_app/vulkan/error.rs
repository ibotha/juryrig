use ash::{vk, LoadingError};
use gpu_allocator::AllocationError;

#[derive(Debug)]
pub enum VulkanError {
    VKErr(vk::Result),
    LoadingError(LoadingError),
    AllocationError(AllocationError),
}

impl From<vk::Result> for VulkanError {
    fn from(value: vk::Result) -> Self {
        VulkanError::VKErr(value)
    }
}

impl From<LoadingError> for VulkanError {
    fn from(value: LoadingError) -> Self {
        VulkanError::LoadingError(value)
    }
}

impl From<AllocationError> for VulkanError {
    fn from(value: AllocationError) -> Self {
        VulkanError::AllocationError(value)
    }
}
