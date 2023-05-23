use rusticnes_ui_common::drawing::{SimpleBuffer, blit, Color};
use image::{self, FilterType};
use super::Background;

pub struct ImageBackground(SimpleBuffer, u8);

impl Background for ImageBackground {
    fn open(path: &str, w: u32, h: u32, alpha: u8) -> Result<Self, String> {
        let dyn_img = image::open(path)
            .map_err(|e| e.to_string())?;
        let img = dyn_img
            .as_rgba8()
            .expect("Convert image");
        let img = image::imageops::resize(img, w, h, FilterType::Gaussian);

        Ok(Self(SimpleBuffer::from_image(img), alpha))
    }

    fn step(&mut self, dest: &mut SimpleBuffer) -> Result<(), String> {
        blit(dest, &self.0, 0, 0, Color::rgba(255, 255, 255, self.1));
        Ok(())
    }
}