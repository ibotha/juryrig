#[repr(C)]
#[derive(Clone, Copy)]
pub struct RGBAPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HDRPixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub struct HDRImage {
    pub width: u32,
    pub height: u32,
    pub(crate) data: Vec<HDRPixel>,
}

pub struct RGBAImage {
    pub width: u32,
    pub height: u32,
    pub(crate) data: Vec<RGBAPixel>,
}

impl RGBAImage {
    pub fn get_pixel(&self, x: u32, y: u32) -> RGBAPixel {
        return self.data[(y * self.width + x) as usize];
    }

    pub fn new(width: u32, height: u32) -> RGBAImage {
        let mut image = RGBAImage {
            width,
            height,
            data: Vec::with_capacity((width * height) as usize),
        };
        unsafe {
            image.data.set_len((width * height) as usize);
        }
        image
    }
}
