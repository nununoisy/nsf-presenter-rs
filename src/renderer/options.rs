use std::str::FromStr;
use std::ffi::OsStr;
use crate::video_builder::SampleFormat;

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
    Duration(u64),
    Loops(usize),
    NsfeLength
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
                    let time = u64::from_str(parts[1]).map_err( | e | e.to_string()) ?;
                    Ok(StopCondition::Duration(time * FRAME_RATE as u64))
                }
            },
            "frames" => {
                let frames = u64::from_str(parts[1]).map_err(|e| e.to_string())?;
                Ok(StopCondition::Duration(frames))
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

impl FromStr for SampleFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "U8" => Ok(SampleFormat::U8),
            "S16" => Ok(SampleFormat::S16),
            "S32" => Ok(SampleFormat::S32),
            "S64" => Ok(SampleFormat::S64),
            "F32" => Ok(SampleFormat::F32),
            "F64" => Ok(SampleFormat::F64),
            "U8P" => Ok(SampleFormat::U8P),
            "S16P" => Ok(SampleFormat::S16P),
            "S32P" => Ok(SampleFormat::S32P),
            "S64P" => Ok(SampleFormat::S64P),
            "FLTP" => Ok(SampleFormat::FLTP),
            "DBLP" => Ok(SampleFormat::DBLP),
            _ => Err(format!("Unknown sample format {}", s))
        }
    }
}

extra_str_traits!(SampleFormat);

#[derive(Clone)]
pub struct RendererOptions {
    pub input_path: String,
    pub output_path: String,

    pub v_codec: String,
    pub a_codec: String,
    pub pix_fmt: String,
    pub sample_fmt: SampleFormat,
    pub sample_rate: i32,

    pub track_index: u8,
    pub stop_condition: StopCondition,
    pub fadeout_length: u64,

    pub ow: u32,
    pub oh: u32,

    pub famicom: bool,
    pub high_quality: bool,
    pub multiplexing: bool,

    pub v_codec_opts: Option<Vec<(String, String)>>,
    pub a_codec_opts: Option<Vec<(String, String)>>,
}
