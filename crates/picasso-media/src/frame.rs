/// An 8-bit straight-alpha RGBA image, rows top to bottom, no padding.
#[derive(Clone, Debug, PartialEq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbaImage {
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let data = rgba.iter().copied().cycle().take(width as usize * height as usize * 4).collect();
        Self { width, height, data }
    }

    pub fn byte_len(width: u32, height: u32) -> usize {
        width as usize * height as usize * 4
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }
}
