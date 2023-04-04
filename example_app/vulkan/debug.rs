use std::ffi::{c_void, CStr};

use log::{log, Level};

use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};

pub(super) struct Debug {
    debug_utils: DebugUtils,
    utils_messenger: vk::DebugUtilsMessengerEXT,
}

impl Debug {
    pub(super) fn new(
        entry: &Entry,
        instance: &Instance,
        debug_create_info: vk::DebugUtilsMessengerCreateInfoEXTBuilder,
    ) -> std::result::Result<Debug, vk::Result> {
        let debug_utils = DebugUtils::new(&entry, &instance);
        let utils_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_create_info, None) }?;
        Ok(Debug {
            debug_utils,
            utils_messenger,
        })
    }

    pub(super) fn create_info() -> vk::DebugUtilsMessengerCreateInfoEXTBuilder<'static> {
        vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            )
            .pfn_user_callback(Some(Self::vulkan_debug_utils_callback))
    }

    unsafe extern "system" fn vulkan_debug_utils_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _p_user_data: *mut c_void,
    ) -> vk::Bool32 {
        let message = CStr::from_ptr((*p_callback_data).p_message);
        let ty = format!("{:?}", message_type).to_lowercase();
        log!(
            match message_severity {
                vk::DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Debug,
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => Level::Warn,
                _ => Level::Info,
            },
            "VK:{} {:?}",
            ty,
            message
        );
        vk::FALSE
    }
}

impl Drop for Debug {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.utils_messenger, None)
        };
    }
}
