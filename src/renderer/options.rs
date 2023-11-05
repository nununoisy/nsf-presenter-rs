use std::collections::HashMap;
use std::str::FromStr;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use rusticnes_ui_common::piano_roll_window::ChannelSettings;
use crate::video_builder::video_options::VideoOptions;

pub const FRAME_RATE: i32 = 60;

macro_rules! extra_str_traits {
    ($t: ty) => {
        impl From<&OsStr> for $t {
            fn from(value: &OsStr) -> Self {
                <$t>::from_str(value.to_str().unwrap()).unwrap()
            }
        }

        impl From<String> for $t {
            fn from(value: String) -> Self {
                <$t>::from_str(value.as_str()).unwrap()
            }
        }
    }
}

#[derive(Copy, Clone)]
pub enum StopCondition {
    Frames(u64),
    Loops(usize),
    NsfeLength
}

impl Display for StopCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StopCondition::Frames(frames) => {
                if (*frames % FRAME_RATE as u64) == 0 {
                    write!(f, "time:{}", *frames / FRAME_RATE as u64)
                } else {
                    write!(f, "frames:{}", *frames)
                }
            },
            StopCondition::Loops(loops) => write!(f, "loops:{}", *loops),
            StopCondition::NsfeLength => write!(f, "time:nsfe")
        }
    }
}

impl FromStr for StopCondition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("Stop condition format invalid, try one of 'time:3', 'time:nsfe', 'frames:180', or 'loops:2'.".to_string());
        }

        match parts[0] {
            "time" => match parts[1] {
                "nsfe" => Ok(StopCondition::NsfeLength),
                _ => {
                    let time = u64::from_str(parts[1]).map_err( | e | e.to_string())?;
                    Ok(StopCondition::Frames(time * FRAME_RATE as u64))
                }
            },
            "frames" => {
                let frames = u64::from_str(parts[1]).map_err(|e| e.to_string())?;
                Ok(StopCondition::Frames(frames))
            },
            "loops" => {
                let loops = usize::from_str(parts[1]).map_err(|e| e.to_string())?;
                Ok(StopCondition::Loops(loops))
            },
            _ => Err(format!("Unknown condition type {}. Valid types are 'time', 'frames', and 'loops'", parts[0]))
        }
    }
}

extra_str_traits!(StopCondition);

#[derive(Clone)]
pub struct RendererOptions {
    pub input_path: String,
    pub video_options: VideoOptions,

    pub track_index: u8,
    pub stop_condition: StopCondition,
    pub fadeout_length: u64,

    pub famicom: bool,
    pub high_quality: bool,
    pub multiplexing: bool,

    pub channel_settings: HashMap<(String, String), ChannelSettings>,
    pub config_import_path: Option<String>
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            input_path: "".to_string(),
            video_options: VideoOptions {
                output_path: "".to_string(),
                metadata: Default::default(),
                background_path: None,
                video_time_base: (29_781, 1_789_773).into(),
                video_codec: "libx264".to_string(),
                video_codec_params: Default::default(),
                pixel_format_in: "rgba".to_string(),
                pixel_format_out: "yuv420p".to_string(),
                resolution_in: (960, 540),
                resolution_out: (1920, 1080),
                audio_time_base: (1, 44_100).into(),
                audio_codec: "aac".to_string(),
                audio_codec_params: Default::default(),
                audio_channels: 1,
                sample_format_in: "s16".to_string(),
                sample_format_out: "fltp".to_string(),
                sample_rate: 44_100,
            },
            track_index: 0,
            stop_condition: StopCondition::Frames(300 * FRAME_RATE as u64),
            fadeout_length: 180,
            famicom: false,
            high_quality: true,
            multiplexing: false,
            channel_settings: HashMap::new(),
            config_import_path: None
        }
    }
}
