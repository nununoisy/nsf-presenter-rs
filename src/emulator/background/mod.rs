mod imagebackground;
mod mtvideobackground;

pub use imagebackground::ImageBackground;
pub use mtvideobackground::MTVideoBackground;
use rusticnes_ui_common::drawing::SimpleBuffer;

pub trait Background {
    fn open(path: &str, w: u32, h: u32, alpha: u8) -> Result<Self, String> where
        Self: Sized;

    fn step(&mut self, dest: &mut SimpleBuffer) -> Result<(), String>;
}
