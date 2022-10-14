#![allow(
    dead_code,
    unused_variables,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps
)]

use vulkanalia::prelude::v1_0::*;

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

const VALIDATION_LAYER: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[vk::KHR_SWAPCHAIN_EXTENSION.name];

mod app;
mod commands;
pub mod errors;
mod frame_buffers;
mod instance;
mod logical_device;
mod physical_device;
mod pipeline;
mod render_pass;
mod structs;
mod swapchain;

pub use app::App;
