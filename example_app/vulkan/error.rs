use ash::{vk, LoadingError};
use gpu_allocator::AllocationError;

#[derive(Debug)]
pub enum RuntimeError {
    VKErr(vk::Result),
    AllocationError(AllocationError),
}

#[derive(Debug)]
// Error enum for issues encountered during initialization.
pub enum InitError {
    // Error propagated directly from Vulkan.
    VKErr(vk::Result),
    LoadingError(LoadingError),
    DeviceSelectionError(&'static str),
    AllocationError(AllocationError),
}

impl From<vk::Result> for InitError {
    fn from(value: vk::Result) -> Self {
        InitError::VKErr(value)
    }
}

impl From<LoadingError> for InitError {
    fn from(value: LoadingError) -> Self {
        InitError::LoadingError(value)
    }
}

impl From<AllocationError> for InitError {
    fn from(value: AllocationError) -> Self {
        InitError::AllocationError(value)
    }
}

impl From<vk::Result> for RuntimeError {
    fn from(value: vk::Result) -> Self {
        RuntimeError::VKErr(value)
    }
}

impl From<AllocationError> for RuntimeError {
    fn from(value: AllocationError) -> Self {
        RuntimeError::AllocationError(value)
    }
}
