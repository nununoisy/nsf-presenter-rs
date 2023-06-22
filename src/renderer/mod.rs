pub mod options;

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crate::emulator;
use crate::video_builder;
use options::{RendererOptions, FRAME_RATE, StopCondition};
use crate::emulator::SongPosition;

pub struct Renderer {
    options: RendererOptions,

    video: video_builder::VideoBuilder,
    emulator: emulator::Emulator,

    encode_start: Instant,
    frame_timestamp: f64,
    frame_times: VecDeque<f64>,
    fadeout_timer: Option<u64>,
    expected_duration: Option<usize>
}

impl Renderer {
    pub fn new(options: RendererOptions) -> Result<Self, String> {
        let mut emulator = emulator::Emulator::new();

        emulator.init(None);
        emulator.open(&options.input_path)?;
        emulator.select_track(options.track_index);
        emulator.config_audio(options.video_options.sample_rate as _, 0x10000, options.famicom, options.high_quality, options.multiplexing);

        let mut video_options = options.video_options.clone();
        video_options.resolution_in = emulator.get_piano_roll_size();

        match emulator.nsf_metadata() {
            Ok(Some((title, artist, copyright))) => {
                video_options.metadata.insert("title".to_string(), title);
                video_options.metadata.insert("artist".to_string(), artist);
                video_options.metadata.insert("album".to_string(), copyright);
                video_options.metadata.insert("track".to_string(), format!("{}/{}", options.track_index, emulator.track_count()));
                video_options.metadata.insert("comment".to_string(), "Encoded with NSFPresenter".to_string());
            },
            _ => ()
        }

        let mut video = video_builder::VideoBuilder::new(video_options)?;

        Ok(Self {
            options: options.clone(),
            video,
            emulator,
            encode_start: Instant::now(),
            frame_timestamp: 0.0,
            frame_times: VecDeque::new(),
            fadeout_timer: None,
            expected_duration: None
        })
    }

    pub fn start_encoding(&mut self) -> Result<(), String> {
        self.encode_start = Instant::now();
        self.video.start_encoding()?;

        // Run for a frame and clear the audio buffer to prevent the pop during initialization
        self.emulator.step();
        self.emulator.clear_sample_buffer();

        Ok(())
    }

    pub fn step(&mut self) -> Result<bool, String> {
        self.emulator.step();

        self.video.push_video_data(&self.emulator.get_piano_roll_frame())?;
        let volume_divisor = match self.fadeout_timer {
            Some(t) => (self.options.fadeout_length as f64 / t as f64) as i16,
            None => 1i16
        };
        if let Some(audio_data) = self.emulator.get_audio_samples(1024, volume_divisor) {
            self.video.push_audio_data(video_builder::as_u8_slice(&audio_data))?;
        }

        self.video.step_encoding()?;

        let elapsed_secs = self.elapsed().as_secs_f64();
        let frame_time = elapsed_secs - self.frame_timestamp;
        self.frame_timestamp = elapsed_secs;

        self.frame_times.push_front(frame_time);
        self.frame_times.truncate(600);

        self.expected_duration = self.next_expected_duration();
        self.fadeout_timer = self.next_fadeout_timer();

        if let Some(t) = self.fadeout_timer {
            if t == 0 {
                return Ok(false)
            }
        }

        Ok(true)
    }

    pub fn finish_encoding(&mut self) -> Result<(), String> {
        self.video.finish_encoding()?;

        Ok(())
    }

    fn next_expected_duration(&self) -> Option<usize> {
        if self.expected_duration.is_some() {
            return self.expected_duration;
        }

        match self.options.stop_condition {
            StopCondition::Frames(stop_duration) => Some((stop_duration + self.options.fadeout_length) as usize),
            StopCondition::Loops(stop_loop_count) => {
                match self.emulator.loop_duration() {
                    Some((s, l)) => Some(self.options.fadeout_length as usize + s + l * stop_loop_count),
                    None => None
                }
            },
            StopCondition::NsfeLength => {
                Some(self.emulator.nsfe_duration().unwrap() + self.options.fadeout_length as usize)
            }
        }
    }

    fn next_fadeout_timer(&self) -> Option<u64> {
        match self.fadeout_timer {
            Some(0) => Some(0),
            Some(t) => Some(t - 1),
            None => {
                let song_ended = match self.emulator.get_song_position() {
                    Some(position) => position.end,
                    None => false
                };
                if song_ended {
                    return Some(self.options.fadeout_length);
                }

                match self.options.stop_condition {
                    StopCondition::Loops(stop_loop_count) => {
                        let loop_count = self.emulator.loop_count()
                            .expect("Loop detection not supported for this NSF");

                        if loop_count >= stop_loop_count {
                            Some(self.options.fadeout_length)
                        } else {
                            None
                        }
                    },
                    StopCondition::Frames(stop_duration) => {
                        if self.cur_frame() >= stop_duration {
                            Some(self.options.fadeout_length)
                        } else {
                            None
                        }
                    },
                    StopCondition::NsfeLength => {
                        let stop_duration = self.emulator.nsfe_duration()
                            .expect("No NSFe/NSF2 duration specified for this track");

                        if self.cur_frame() >= stop_duration as u64 {
                            Some(self.options.fadeout_length)
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }

    pub fn cur_frame(&self) -> u64 {
        self.emulator.last_frame() as u64
    }

    pub fn elapsed(&self) -> Duration {
        self.encode_start.elapsed()
    }

    pub fn instantaneous_fps(&self) -> u32 {
        let frame_time = match self.frame_times.front() {
            Some(t) => t.clone(),
            None => 1.0
        };
        (1.0 / frame_time) as u32
    }

    pub fn average_fps(&self) -> u32 {
        if self.frame_times.is_empty() {
            return 0;
        }
        (self.frame_times.len() as f64 / self.frame_times.iter().sum::<f64>()) as u32
    }

    pub fn encode_rate(&self) -> f64 {
        self.average_fps() as f64 / emulator::NES_NTSC_FRAMERATE
    }

    pub fn encoded_duration(&self) -> Duration {
        self.video.encoded_video_duration()
    }

    pub fn encoded_size(&self) -> usize {
        self.video.encoded_video_size()
    }

    pub fn expected_duration_frames(&self) -> Option<usize> {
        self.expected_duration
    }

    pub fn expected_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.expected_duration? as f64 / emulator::NES_NTSC_FRAMERATE))
    }

    pub fn eta_duration(&self) -> Option<Duration> {
        match self.expected_duration {
            Some(expected_duration) => {
                let remaining_frames = expected_duration - self.cur_frame() as usize;
                let remaining_secs = remaining_frames as f64 / emulator::NES_NTSC_FRAMERATE;
                Some(Duration::from_secs_f64(self.elapsed().as_secs_f64() + remaining_secs))
            },
            None => None
        }
    }

    pub fn emulator_progress(&self) -> Result<String, String> {
        self.emulator.progress()
    }

    pub fn song_position(&self) -> Option<SongPosition> {
        self.emulator.get_song_position()
    }

    pub fn loop_count(&self) -> Option<usize> {
        self.emulator.loop_count()
    }
}
