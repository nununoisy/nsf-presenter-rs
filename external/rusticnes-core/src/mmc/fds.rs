use std::f32::consts::PI;

use apu::{AudioChannelState, FilterChain};
use apu::PlaybackRate;
use apu::Volume;
use apu::RingBuffer;
use apu::filters;
use apu::filters::DspFilter;

pub struct LowPassRC {
    pub accumulator: f32,
    pub alpha: f32
}

impl LowPassRC {
    pub fn new(sample_rate: f32, cutoff: f32) -> Self {
        Self {
            accumulator: 0.0,
            alpha: (-2.0 * PI * cutoff / sample_rate).exp()
        }
    }
}

impl DspFilter for LowPassRC {
    fn consume(&mut self, new_input: f32) {
        self.accumulator = (self.alpha * self.accumulator) + ((1.0 - self.alpha) * new_input);
    }

    fn output(&self) -> f32 {
        return self.accumulator;
    }
}

pub struct FdsModTable {
    pub table: [u8; 64],
    pub frequency: usize,
    pub phase: usize,
    pub mod_halt: bool,
    pub pos: u32,
    pub write_pos: u32
}

impl FdsModTable {
    pub fn new() -> Self {
        Self {
            table: [0u8; 64],
            frequency: 0,
            phase: 0,
            mod_halt: true,
            pos: 0,
            write_pos: 0
        }
    }

    pub fn clock(&mut self) {
        if self.mod_halt {
            return;
        }

        let start_pos = self.phase >> 16;
        self.phase += self.frequency;
        let end_pos = self.phase >> 16;

        self.phase &= 0x3FFFFF;

        for pos in start_pos..end_pos {
            let wave = self.table[pos & 0x3F];
            match wave {
                0 => (),
                1 => self.pos += 1,
                2 => self.pos += 2,
                3 => self.pos += 4,
                4 => self.pos = 0,
                5 => self.pos -= 4,
                6 => self.pos -= 2,
                7 => self.pos -= 1,
                n => panic!("Invalid wavetable entry {}", n)
            };
            self.pos &= 0x7F;
        }
    }

    pub fn write_freq_low(&mut self, freq_low: u8) {
        self.frequency = (self.frequency & 0xF00) | (freq_low as usize);
    }

    pub fn write_freq_high(&mut self, freq_high: u8) {
        self.frequency = (self.frequency & 0xFF) | ((freq_high as usize) << 8);
        self.mod_halt = (freq_high & 0x80) != 0;

        if self.mod_halt {
            self.phase &= 0x3F0000;
        }
    }
}

pub struct FdsEnvelope {
    pub mode: bool,
    pub disable: bool,
    pub timer: u32,
    pub speed: u8,
    pub out: u8
}

impl FdsEnvelope {
    pub fn new() -> Self {
        Self {
            mode: false,
            disable: true,
            timer: 0,
            speed: 0,
            out: 0
        }
    }

    pub fn clock(&mut self, master_envelope_speed: u8) {
        if self.disable {
            return;
        }

        self.timer += 1;
        let period = ((self.speed as u32 + 1) * (master_envelope_speed as u32)) << 3;

        while self.timer >= period {
            if self.mode && self.out < 32 {
                self.out += 1;
            } else if !self.mode && self.out > 0 {
                self.out -= 1;
            }
            self.timer -= period;
        }
    }

    pub fn write_config_register(&mut self, config: u8) {
        self.speed = config & 0x3F;
        self.timer = 0;
        self.disable = (config & 0x80) != 0;
        self.mode = (config & 0x40) != 0;

        if self.disable {
            self.out = self.speed;
        }
    }
}

pub struct FdsWaveTable {
    pub table: [u8; 64],
    pub frequency: u16,
    pub phase: usize,
    pub write_enable: bool,
    pub master_volume: u8,
    pub wave_halt: bool,
    pub env_halt: bool,
    pub env_halt_ticked: bool,
    pub tick_frequency: f32
}

impl FdsWaveTable {
    pub fn new() -> Self {
        Self {
            table: [0u8; 64],
            frequency: 0,
            phase: 0,
            write_enable: false,
            master_volume: 0,
            wave_halt: true,
            env_halt: true,
            env_halt_ticked: false,
            tick_frequency: 0.0
        }
    }

    pub fn clock(&mut self, mod_pos: u32, mod_out: u8) {
        self.env_halt_ticked = false;

        if self.wave_halt {
            return;
        }

        let mod_quantity = match mod_out {
            0 => 0i32,
            out => {
                let pos7 = (mod_pos - ((mod_pos & 0x40) << 1)) as i32;
                let mut mod_quantity = pos7 * out as i32;

                let mut rem = mod_quantity & 0x0F;
                mod_quantity >>= 4;

                if rem > 0 && (mod_quantity & 0x80) == 0 {
                    if pos7 < 0 {
                        mod_quantity -= 1;
                    } else {
                        mod_quantity += 2;
                    }
                }

                while mod_quantity >= 192 { mod_quantity -= 256; }
                while mod_quantity <  -64 { mod_quantity += 256; }

                mod_quantity *= self.frequency as i32;

                rem = mod_quantity & 0x3F;
                mod_quantity >>= 6;

                if rem >= 32 {
                    mod_quantity += 1;
                }

                mod_quantity
            }
        };

        self.phase = (self.phase as i32 + self.frequency as i32 + mod_quantity) as usize;
        self.phase &= 0x3FFFFF;

        self.tick_frequency = (self.frequency as i32 + mod_quantity) as f32
    }

    pub fn write_freq_low(&mut self, freq_low: u8) {
        self.frequency = (self.frequency & 0xF00) | (freq_low as u16);
    }

    pub fn write_freq_high(&mut self, freq_high: u8) {
        self.frequency = (self.frequency & 0xFF) | ((freq_high as u16) << 8);
        self.wave_halt = (freq_high & 0x80) != 0;
        self.env_halt = (freq_high & 0x40) != 0;
        if self.wave_halt {
            self.phase = 0;
        }
        if self.env_halt {
            self.env_halt_ticked = true;
        }
    }

    pub fn write_config_register(&mut self, config: u8) {
        self.write_enable = (config & 0x80) != 0;
        self.master_volume = config & 0x3;
    }
}

fn build_output_filter(sample_rate: f32) -> FilterChain {
    let mut chain = FilterChain::new();

    // The FDS has a 1-pole RC low-pass filter on the output, with a cutoff around 2kHz.
    // However, as a result of some of the downsampling further down the render chain, unwanted
    // noise can be introduced when modulation is used. An additional low-pass is added here to
    // filter away the modulation noise so it better matches other emulators, though I'm not sure
    // how accurate this is to the real hardware.
    chain.add(Box::new(LowPassRC::new(1789773.0, 2000.0)), 1789773.0);
    chain.add(Box::new(LowPassRC::new(sample_rate, 6000.0)), sample_rate);

    chain
}

pub struct FdsChannel {
    pub name: String,
    pub debug_disable: bool,

    pub wave_table: FdsWaveTable,
    pub mod_table: FdsModTable,
    pub vol_envelope: FdsEnvelope,
    pub mod_envelope: FdsEnvelope,

    pub master_envelope_speed: u8,

    pub current_volume: f32,
    pub output_filter: FilterChain,

    pub output_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub last_edge: bool,
    pub debug_filter: filters::HighPassIIR
}

impl FdsChannel {
    pub fn new(channel_name: &str) -> Self {
        Self {
            name: channel_name.to_string(),
            debug_disable: false,

            wave_table: FdsWaveTable::new(),
            mod_table: FdsModTable::new(),
            vol_envelope: FdsEnvelope::new(),
            mod_envelope: FdsEnvelope::new(),

            master_envelope_speed: 0xFF,

            current_volume: 0.0,
            output_filter: build_output_filter(44100.0),

            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0)
        }
    }

    pub fn nsf_init(&mut self) {
        // FDS BIOS: $4080 <- 0x80
        self.vol_envelope.write_config_register(0x80);
        // FDS BIOS: $408A <- 0xE8
        self.master_envelope_speed = 0xE8;
        // $4082 <- 0
        self.wave_table.write_freq_low(0);
        // $4083 <- 0x80
        self.wave_table.write_freq_high(0x80);
        // $4084 <- 0x80
        self.mod_envelope.write_config_register(0x80);
        // $4085 <- 0
        self.mod_table.pos = 0;
        // $4086 <- 0
        self.mod_table.write_freq_low(0);
        // $4087 <- 0x80
        self.mod_table.write_freq_high(0x80);
        // $4089 <- 0
        self.mod_envelope.write_config_register(0);
    }

    pub fn clock(&mut self) {
        if self.wave_table.env_halt_ticked {
            self.mod_envelope.timer = 0;
            self.vol_envelope.timer = 0;
        }

        if !self.wave_table.wave_halt && !self.wave_table.env_halt && self.master_envelope_speed != 0 {
            self.mod_envelope.clock(self.master_envelope_speed);
            self.vol_envelope.clock(self.master_envelope_speed);
        }

        let old_wave_idx = (self.wave_table.phase >> 16) & 0x3F;

        self.mod_table.clock();
        self.wave_table.clock(self.mod_table.pos, self.mod_envelope.out);

        let mut vol_out = self.vol_envelope.out as i32;
        if vol_out > 32 {
            vol_out = 32;
        }

        if !self.wave_table.write_enable {
            let wave_idx = (self.wave_table.phase >> 16) & 0x3F;
            let wave = self.wave_table.table[wave_idx] as i32 * vol_out;

            let volume = (wave * 2 / (self.wave_table.master_volume as i32 + 2)) as f32;
            self.output_filter.consume(volume, 1.0 / 1789773.0);
            self.current_volume = self.output_filter.output();
            self.last_edge = old_wave_idx > wave_idx;
        }
    }
}

impl AudioChannelState for FdsChannel {
    fn name(&self) -> String {
        return self.name.clone();
    }

    fn chip(&self) -> String {
        return "FDS".to_string();
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.output_buffer;
    }

    fn edge_buffer(&self) -> &RingBuffer {
        return &self.edge_buffer;
    }

    fn record_current_output(&mut self) {
        self.debug_filter.consume(self.current_volume);
        self.output_buffer.push((self.debug_filter.output() * -1.2) as i16);
        self.edge_buffer.push(self.last_edge as i16);
        self.last_edge = false;
    }

    fn min_sample(&self) -> i16 {
        -2048
    }

    fn max_sample(&self) -> i16 {
        2048
    }

    fn muted(&self) -> bool {
        self.debug_disable
    }

    fn mute(&mut self) {
        self.debug_disable = true;
    }

    fn unmute(&mut self) {
        self.debug_disable = false;
    }

    fn playing(&self) -> bool {
        true
    }

    fn rate(&self) -> PlaybackRate {
        let frequency = (1_789_773.0 / 65535.0) * (self.wave_table.tick_frequency / 64.0);
        return PlaybackRate::FundamentalFrequency {frequency: frequency};
    }

    fn volume(&self) -> Option<Volume> {
        return Some(Volume::VolumeIndex{ index: self.current_volume as usize, max: 2048 });
    }
}
