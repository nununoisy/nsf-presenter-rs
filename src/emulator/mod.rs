mod nsf;
mod nsfeparser;
mod background;
mod emulator;

use std::fmt::{Display, Formatter};

pub use emulator::Emulator;
pub use nsf::NsfDriverType;
pub const NES_NTSC_FRAMERATE: f64 = 1789772.7272727 / 29780.5;
pub const NES_PAL_FRAMERATE: f64 = 1662607.0 / 33247.5;

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
struct SongPosition {
    pub frame: u8,
    pub row: u8
}

impl SongPosition {
    pub fn new(frame: u8, row: u8) -> Self {
        Self {
            frame,
            row
        }
    }
}

impl Display for SongPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}:{:02X}", self.frame, self.row)
    }
}

const DEFAULT_CONFIG: &str = r###"
[piano_roll]
background_color = "rgba(0, 0, 0, 0)"
canvas_width = 960
canvas_height = 540
key_length = 24
key_thickness = 5
octave_count = 9
scale_factor = 1
speed_multiplier = 1
starting_octave = 0
waveform_height = 48
oscilloscope_glow_thickness = 2.0
oscilloscope_line_thickness = 0.75
draw_piano_strings = false
"###;
