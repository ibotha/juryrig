use ash::{Entry, Instance, vk, extensions::khr};
use winit::window::Window;

#[cfg(target_family = "windows")]
use {ash::extensions::khr::Win32Surface, winit::platform::windows::WindowExtWindows};

#[cfg(target_family = "unix")]
use {ash::extensions::khr::XlibSurface, winit::platform::x11::WindowExtX11};

pub(super) struct Surface {
    loader: khr::Surface,
    pub(super) surface: vk::SurfaceKHR,
}

impl Surface {
    pub (super) fn new(window: &Window, entry: &Entry, instance: &Instance) -> Result<Surface, vk::Result> {
        #[cfg(target_family = "windows")]
        fn create_surface(
            window: &Window,
            entry: &Entry,
            instance: &Instance,
        ) -> std::result::Result<vk::SurfaceKHR, vk::Result> {
            use std::ffi::c_void;

            let hwnd = window.hwnd();
            let hinstance = window.hinstance();
            let win32_surface_create_info = vk::Win32SurfaceCreateInfoKHR::builder()
                .hwnd(hwnd as *const c_void)
                .hinstance(hinstance as *const c_void);
            let win32_surface_loader = khr::Win32Surface::new(&entry, &instance);
            return unsafe {
                win32_surface_loader.create_win32_surface(&win32_surface_create_info, None)
            };
        }

        #[cfg(target_family = "unix")]
        fn create_surface(
            window: &Window,
            entry: &Entry,
            instance: &Instance,
        ) -> std::result::Result<vk::SurfaceKHR, vk::Result> {
            let x11_display = window.xlib_display().unwrap();
            let x11_window = window.xlib_window().unwrap();
            let x11_create_info = vk::XlibSurfaceCreateInfoKHR::builder()
                .window(x11_window)
                .dpy(x11_display as *mut vk::Display);
            let xlib_surface_loader = khr::XlibSurface::new(&entry, &instance);
            unsafe { xlib_surface_loader.create_xlib_surface(&x11_create_info, None) }
        }

        let surface = create_surface(&window, &entry, &instance)?;

        let surface_loader = khr::Surface::new(&entry, &instance);
        Ok(Surface {
            surface,
            loader: surface_loader,
        })
    }

    pub(super) fn get_physical_device_surface_support(
        &self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Result<bool, vk::Result> {
        unsafe {
            self.loader.get_physical_device_surface_support(
                physical_device,
                queue_family_index,
                self.surface,
            )
        }
    }

    pub(crate) fn get_capabilities(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> std::result::Result<vk::SurfaceCapabilitiesKHR, ash::vk::Result> {
        unsafe {
            self.loader
                .get_physical_device_surface_capabilities(physical_device, self.surface)
        }
    }

    pub(crate) fn get_present_modes(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> std::result::Result<Vec<vk::PresentModeKHR>, ash::vk::Result> {
        unsafe {
            self.loader
                .get_physical_device_surface_present_modes(physical_device, self.surface)
        }
    }

    pub(crate) fn get_formats(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Vec<vk::SurfaceFormatKHR>, vk::Result> {
        unsafe {
            self.loader
                .get_physical_device_surface_formats(physical_device, self.surface)
        }
    }

    pub(super) fn extention_name_ptr() -> *const i8 {
        #[cfg(target_family = "windows")]
        return Win32Surface::name().as_ptr();

        #[cfg(target_family = "unix")]
        return XlibSurface::name().as_ptr();
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.surface, None) }
    }
}