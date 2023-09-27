extern crate rusticnes_core;
extern crate rusticnes_ui_common;

use std::collections::HashMap;
use std::collections::vec_deque::VecDeque;
use std::fs;
use std::str;
use std::rc::Rc;
use rusticnes_core::apu::FilterType;
use rusticnes_ui_common::application::RuntimeState as RusticNESRuntimeState;
use rusticnes_ui_common::drawing::{blit, Color, SimpleBuffer};
use rusticnes_ui_common::events::Event;
use rusticnes_ui_common::panel::Panel;
use rusticnes_ui_common::piano_roll_window::{ChannelSettings, PianoRollWindow, PollingType};
use rusticnes_ui_common::settings::SettingsState;
use super::{SongPosition, DEFAULT_CONFIG};
use super::nsf::{Nsf, NsfDriverType};
use super::nsfeparser::NsfeMetadata;

pub struct Emulator {
    runtime: RusticNESRuntimeState,
    nsf: Option<Nsf>,
    nsf_track_index: u8,
    nsfe_metadata: Option<NsfeMetadata>,
    event_queue: VecDeque<Event>,
    piano_roll_window: PianoRollWindow,
    sample_buffer: VecDeque<i16>,
    song_positions: HashMap<SongPosition, u32>,
    last_position: Option<SongPosition>,
    loop_duration: Option<(usize, usize)>,
    loop_count: usize
}

impl Emulator {
    pub fn new() -> Emulator {
        Emulator {
            runtime: RusticNESRuntimeState::new(),
            nsf: None,
            nsf_track_index: 1,
            nsfe_metadata: None,
            event_queue: VecDeque::new(),
            piano_roll_window: PianoRollWindow::new(),
            sample_buffer: VecDeque::new(),
            song_positions: HashMap::new(),
            last_position: None,
            loop_duration: None,
            loop_count: 0
        }
    }

    pub fn driver_type(&self) -> NsfDriverType {
        match &self.nsf {
            Some(nsf) => nsf.driver_type(),
            None => NsfDriverType::Unknown
        }
    }

    fn _dispatch(&mut self) {
        while let Some(event) = self.event_queue.pop_front() {
            self.event_queue.extend(self.piano_roll_window.handle_event(&self.runtime, event.clone()));
            self.event_queue.extend(self.runtime.handle_event(event.clone()));
        };
    }

    fn dispatch(&mut self, event: Event) {
        self.event_queue.push_back(event);
        self._dispatch();
    }

    pub fn init(&mut self) {
        let default_settings = SettingsState::new();
        let default_settings_events = default_settings.apply_settings();
        self.event_queue.extend(default_settings_events);
        self._dispatch();

        self.runtime.settings.load_str(DEFAULT_CONFIG);
        let settings_events = self.runtime.settings.apply_settings();
        self.event_queue.extend(settings_events);
        self._dispatch();
    }

    fn load(&mut self, cart_data: &[u8]) {
        let empty_vec: Vec<u8> = Vec::new();
        let data_vec: Vec<u8> = cart_data.to_vec();

        let nsf = Nsf::from(&cart_data);
        if nsf.magic_valid() {
            self.nsf = Some(nsf);
            self.nsfe_metadata = self.nsf.as_ref().unwrap().nsfe_metadata();
        }

        self.dispatch(Event::LoadCartridge("cartridge".to_string(), Rc::new(data_vec), Rc::new(empty_vec)));

        if self.nsf.is_some() {
            println!("NSF Version: {}", self.nsf.as_ref().unwrap().version());
            println!("Title: {}", self.nsf.as_ref().unwrap().title().unwrap());
            println!("Artist: {}", self.nsf.as_ref().unwrap().artist().unwrap());
            println!("Copyright: {}", self.nsf.as_ref().unwrap().copyright().unwrap());

            print!("Chips: 2A03");
            if self.nsf.as_ref().unwrap().fds() { print!(", FDS"); }
            if self.nsf.as_ref().unwrap().n163() { print!(", N163"); }
            if self.nsf.as_ref().unwrap().mmc5() { print!(", MMC5"); }
            if self.nsf.as_ref().unwrap().vrc6() { print!(", VRC6"); }
            if self.nsf.as_ref().unwrap().vrc7() { print!(", VRC7"); }
            if self.nsf.as_ref().unwrap().s5b() { print!(", S5B"); }
            println!();

            match self.driver_type() {
                NsfDriverType::Unknown => println!("Driver type: unknown"),
                NsfDriverType::FTClassic => println!("Driver type: classic FamiTracker"),
                NsfDriverType::FT0CC => println!("Driver type: 0CC-FamiTracker"),
                NsfDriverType::FTDn => println!("Driver type: Dn-FamiTracker")
            }
        }
    }

    pub fn open(&mut self, path: &str) -> Result<(), String> {
        let cart_data = fs::read(path)
            .map_err(|err| format!("Failed to read input file {}: {}", path, err))?;
        self.load(&cart_data);
        Ok(())
    }

    pub fn select_track(&mut self, index: u8) {
        if index > 0 && index <= self.track_count() {
            self.nsf_track_index = index;
            self.runtime.nes.mapper.nsf_set_track(index);
            self.runtime.nes.mapper.nsf_manual_mode();
        }
    }

    pub fn track_count(&self) -> u8 {
        match &self.nsf {
            Some(nsf) => nsf.songs(),
            None => 1
        }
    }

    pub fn nsf_metadata(&self) -> Result<Option<(String, String, String)>, String> {
        Ok(match (&self.nsf, &self.nsfe_metadata) {
            (None, _) => None,
            (Some(nsf), None) => Some({
                let title = nsf.title()?;
                let artist = nsf.artist()?;
                let copyright = nsf.copyright()?;
                (title, artist, copyright)
            }),
            (Some(nsf), Some(nsfe_metadata)) => Some({
                let title = nsfe_metadata.track_title(self.nsf_track_index as _)
                    .unwrap_or(nsfe_metadata.title().unwrap_or(nsf.title()?));
                let artist = nsfe_metadata.track_author(self.nsf_track_index as _)
                    .unwrap_or(nsfe_metadata.artist().unwrap_or(nsf.artist()?));
                let copyright = nsfe_metadata.copyright().unwrap_or(nsf.title()?);
                (title, artist, copyright)
            })
        })
    }

    fn get_famitracker_song_position(&self, mut ptr: usize) -> SongPosition {
        if let Some(nsf) = &self.nsf {
            // TODO: this is sometimes needed but other times not? Needs further investigation
            // if nsf.vrc7() {
            //     ptr += 1;
            // }
            if nsf.fds() {
                ptr += 2;
            }
        }

        let player_flags = self.runtime.nes.memory.iram_raw[ptr];
        let row = self.runtime.nes.memory.iram_raw[ptr+1];
        let frame = self.runtime.nes.memory.iram_raw[ptr+2];
        let engine_flags = self.runtime.nes.memory.iram_raw[ptr+3];

        if (player_flags & 0x2) != 0 {
            // If a Cxx was issued, report that the song has ended.
            SongPosition::at_end()
        } else if (engine_flags & 0x1) != 0 {
            // If the engine is loading the next frame, the row number will be wrong - correct it
            SongPosition::new(frame, 0)
        } else {
            SongPosition::new(frame, row)
        }
    }

    pub fn get_song_position(&self) -> Option<SongPosition> {
        match self.driver_type() {
            NsfDriverType::FTClassic => Some(self.get_famitracker_song_position(0x211)),
            NsfDriverType::FT0CC => Some(self.get_famitracker_song_position(0x215)),
            NsfDriverType::FTDn => Some(self.get_famitracker_song_position(0x215)),
            NsfDriverType::Unknown => None
        }
    }

    pub fn step(&mut self) {
        while self.runtime.nes.ppu.current_scanline == 242 {
            self.dispatch(Event::NesRunScanline);
        }
        while self.runtime.nes.ppu.current_scanline != 242 {
            self.dispatch(Event::NesRunScanline);
        }
        self.dispatch(Event::Update);

        if let Some(position) = self.get_song_position() {
            let last_frame = self.last_frame();

            if self.song_positions.contains_key(&position) {
                if let Some(last_position) = self.last_position {
                    if position.frame < last_position.frame {
                        self.loop_count += 1;
                    }
                }

                if self.loop_duration.is_none() {
                    let start_frame = self.song_positions.get(&position).cloned();
                    let end_frame = Some(last_frame);

                    self.loop_duration = match (start_frame, end_frame) {
                        (Some(start), Some(end)) => {
                            if end - start >= 60 {
                                Some((start as usize, (end - start) as usize))
                            } else {
                                None
                            }
                        },
                        _ => None
                    }
                }
            } else {
                self.song_positions.insert(position, last_frame);
            }

            self.last_position = Some(position);
        }
    }

    pub fn get_piano_roll_size(&self) -> (u32, u32) {
        (self.piano_roll_window.active_canvas().width, self.piano_roll_window.active_canvas().height)
    }

    pub fn get_piano_roll_frame(&mut self) -> Vec<u8> {
        self.dispatch(Event::RequestFrame);

        self.piano_roll_window.active_canvas().buffer.clone()
    }

    pub fn config_audio(&mut self, sample_rate: u64, buffer_size: usize, famicom: bool, high_quality: bool, multiplexing: bool) {
        self.runtime.nes.apu.set_sample_rate(sample_rate);

        let ft = match famicom {
            true => FilterType::FamiCom,
            false => FilterType::Nes
        };
        self.runtime.nes.apu.set_filter(ft, high_quality);
        self.runtime.nes.apu.set_buffer_size(buffer_size);
        self.runtime.nes.mapper.audio_multiplexing(multiplexing);

        self.dispatch(Event::Update);

        if self.sample_buffer.capacity() < buffer_size {
            self.sample_buffer.reserve(buffer_size);
        }

        self.piano_roll_window.polling_type = PollingType::ApuQuarterFrame;
    }

    pub fn get_audio_samples(&mut self, sample_count: usize, volume_divisor: i16) -> Option<Vec<i16>> {
        if self.runtime.nes.apu.samples_queued() < 256 {
            return None;
        }

        let samples: Vec<i16> = self.runtime.nes.apu.consume_samples();
        self.sample_buffer.extend(samples);

        if self.sample_buffer.len() < sample_count {
            return None;
        }

        let volume_divisor = match volume_divisor {
            0 => 1,
            v => v
        };

        let samples: Vec<i16> = self.sample_buffer
            .drain(0..sample_count)
            .map(|s| s / volume_divisor)
            .map(|s| s.saturating_add(s / 3))
            .collect();
        Some(samples)
    }

    pub fn clear_sample_buffer(&mut self) {
        self.sample_buffer.clear();
    }

    pub fn last_frame(&self) -> u32 {
        self.runtime.nes.last_frame
    }

    pub fn loop_count(&self) -> Option<usize> {
        match self.driver_type() {
            NsfDriverType::Unknown => None,
            _ => Some(self.loop_count)
        }
    }

    pub fn nsfe_duration(&self) -> Option<usize> {
        self.nsfe_metadata.as_ref()?.track_duration(self.nsf_track_index as _).clone()
    }

    pub fn nsfe_fadeout(&self) -> Option<usize> {
        self.nsfe_metadata.as_ref()?.track_fadeout(self.nsf_track_index as _).clone()
    }

    pub fn loop_duration(&self) -> Option<(usize, usize)> {
        self.loop_duration
    }

    fn driver_progress(&self) -> Option<String> {
        let result = match self.get_song_position() {
            Some(position) => format!("pos={} loop={}", position, self.loop_count),
            None => format!("pos=? loop={}", self.loop_count)
        };
        Some(result)
    }

    pub fn progress(&self) -> Result<String, String> {
        let generic_progress = format!("frame={}", self.runtime.nes.last_frame);

        let result = match self.driver_progress() {
            Some(driver_progress) => format!("{} {}", generic_progress, driver_progress),
            None => generic_progress
        };
        Ok(result)
    }

    pub fn channel_settings(&self) -> HashMap<(String, String), ChannelSettings> {
        let mut result: HashMap<(String, String), ChannelSettings> = HashMap::new();

        for (chip, channels) in self.piano_roll_window.channel_settings.iter() {
            for (channel, settings) in channels {
                result.insert((chip.clone(), channel.clone()), settings.clone());
            }
        }

        result
    }

    pub fn apply_channel_settings(&mut self, settings: &HashMap<(String, String), ChannelSettings>) {
        for ((chip, channel), channel_settings) in settings.iter() {
            self.piano_roll_window.channel_settings
                .entry(chip.clone())
                .and_modify(|chip_settings| {
                    if chip_settings.contains_key(channel) {
                        chip_settings.insert(channel.clone(), channel_settings.clone());
                    }
                });
        }
    }
}
