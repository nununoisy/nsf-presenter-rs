mod nsf;
mod nsfeparser;
mod emulator;
pub mod m3u_searcher;
mod config;

use std::fmt::{Display, Formatter};

pub use emulator::Emulator;
pub use nsf::{Nsf, NsfDriverType};
pub const NES_NTSC_FRAMERATE: f64 = 1789772.7272727 / 29780.5;
// pub const NES_PAL_FRAMERATE: f64 = 1662607.0 / 33247.5;

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
pub struct SongPosition {
    pub end: bool,
    pub frame: u8,
    pub row: u8
}

impl SongPosition {
    pub fn new(frame: u8, row: u8) -> Self {
        Self {
            end: false,
            frame,
            row
        }
    }

    pub fn at_end() -> Self {
        Self {
            end: true,
            frame: 0,
            row: 0
        }
    }
}

impl Display for SongPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}:{:02X}", self.frame, self.row)
    }
}
