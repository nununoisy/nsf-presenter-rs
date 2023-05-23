use mmc::mapper::Mapper;

use std::fs::OpenOptions;
use std::io::prelude::*;

mod audio_channel;
mod dmc;
pub mod filters;
mod length_counter;
mod noise;
mod pulse;
mod ring_buffer;
mod triangle;
mod volume_envelope;

pub use self::audio_channel::AudioChannelState;
pub use self::audio_channel::PlaybackRate;
pub use self::audio_channel::Volume;
pub use self::audio_channel::Timbre;
pub use self::dmc::DmcState;
pub use self::noise::NoiseChannelState;
pub use self::pulse::PulseChannelState;
pub use self::ring_buffer::RingBuffer;
pub use self::triangle::TriangleChannelState;

pub use self::filters::DspFilter;
pub use self::filters::FilterChain;

#[derive(Clone, Copy)]
pub enum FilterType {
    Nes,
    FamiCom,
}

pub struct ApuState {
    pub current_cycle: u64,

    pub frame_sequencer_mode: u8,
    pub frame_sequencer: u16,
    pub frame_reset_delay: u8,
    pub quarter_frame_counter: u32,
    pub half_frame_counter: u32,

    pub frame_interrupt: bool,
    pub disable_interrupt: bool,

    pub pulse_1: PulseChannelState,
    pub pulse_2: PulseChannelState,
    pub triangle: TriangleChannelState,
    pub noise: NoiseChannelState,
    pub dmc: DmcState,

    pub staging_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub output_buffer: Vec<i16>,
    pub buffer_full: bool,
    pub sample_rate: u64,
    pub cpu_clock_rate: u64,
    pub generated_samples: u64,
    pub next_sample_at: u64,

    // Lookup tables for emulating the mixer
    pub pulse_table: Vec<f32>,
    pub tnd_table: Vec<f32>,

    // filter chain (todo: make this a tad more flexible)
    // also todo: make sure these are recreated when changing sample rate

    pub filter_type: FilterType,
    pub filter_chain: FilterChain,
    pub filter_hq: bool,
}

fn generate_pulse_table() -> Vec<f32> {
    let mut pulse_table = vec!(0f32; 31);
    for n in 1 .. 31 {
        pulse_table[n] = 95.52 / (8128.0 / (n as f32) + 100.0);
    }
    return pulse_table;
}

fn full_tnd_index(t: usize, n: usize, d: usize) -> usize {
    return (d * 16 * 16) + (n * 16) + t;
}

fn generate_tnd_table() -> Vec<f32> {
    let mut tnd_table = vec!(0f32; 16*16*128);
    for tri in 0 .. 16 {
        for noise in 0 .. 16 {
            for dmc in 0 .. 128 {
                if tri + noise + dmc > 0 {
                    let i = full_tnd_index(tri, noise, dmc);
                    tnd_table[i] = 159.79 / ((1.0 / ((tri as f32 / 8227.0) + (noise as f32 / 12241.0) + (dmc as f32 / 22638.0))) + 100.0);
                }
            }
        }
    }
    return tnd_table;
}

fn recommended_buffer_size(sample_rate: u64) -> usize {
    let samples_per_frame = sample_rate / 60;
    let mut buffer_size = 1;
    // Because most audio hardware will prefer a power of 2 buffer size, find the smallest
    // one of those that is large enough to house all the samples we could generate in
    // a single frame
    while buffer_size < samples_per_frame {
        buffer_size = buffer_size * 2;
    }
    return buffer_size as usize;
}

fn construct_hq_filter_chain(clock_rate: f32, target_sample_rate: f32, filter_type: FilterType) -> FilterChain {
    // https://wiki.nesdev.org/w/index.php?title=APU_Mixer

    // First, no matter what the hardware specifies, we'll do a lightweight downsample to around 8x
    // the target sample rate. This is to somewhat reduce the CPU cost of the rest of the chain
    let mut chain = FilterChain::new();
    let intermediate_samplerate = target_sample_rate * (2.0 + (std::f32::consts::PI / 32.0));
    let intermediate_cutoff_frequency = target_sample_rate * 0.4;
    // This IIR isn't especially sharp, but that's okay. We'll do a better filter later
    // to deal with any aliasing this leaves behind
    chain.add(Box::new(filters::LowPassIIR::new(clock_rate, intermediate_cutoff_frequency)), clock_rate);

    match filter_type {
        FilterType::Nes => {
            //The NES hardware follows the DACs with a surprisingly involved circuit that adds several low-pass and high-pass filters:

            // A first-order high-pass filter at 90 Hz
            chain.add(Box::new(filters::HighPassIIR::new(intermediate_samplerate, 90.0)), intermediate_samplerate);
            //  Another first-order high-pass filter at 440 Hz
            chain.add(Box::new(filters::HighPassIIR::new(intermediate_samplerate, 440.0)), intermediate_samplerate);
            // A first-order low-pass filter at 14 kHz
            chain.add(Box::new(filters::LowPassIIR::new(intermediate_samplerate, 14000.0)), intermediate_samplerate);
        },
        FilterType::FamiCom => {
            // The Famicom hardware instead ONLY specifies a first-order high-pass filter at 37 Hz, 
            // followed by the unknown (and varying) properties of the RF modulator and demodulator. 
            chain.add(Box::new(filters::HighPassIIR::new(intermediate_samplerate, 37.0)), intermediate_samplerate);
        }
    }

    // Finally, perform a high-quality low pass, the result of which will be decimated to become the final output
    // TODO: 160 is huge! That was needed when going from 1.7 MHz -> 44.1 kHz; is it still needed when the source
    // is more like 88.2 kHz? Figure out if we can lower this, it's very expensive.
    let window_size = 16;
    let cutoff_frequency = target_sample_rate * 0.45;
    chain.add(Box::new(filters::LowPassFIR::new(intermediate_samplerate, cutoff_frequency, window_size)), intermediate_samplerate);

    return chain;
}

fn construct_lq_filter_chain(clock_rate: f32, target_sample_rate: f32, filter_type: FilterType) -> FilterChain {
    // https://wiki.nesdev.org/w/index.php?title=APU_Mixer

    // Quicker and more dirty. Will sound somewhat muffled.
    let mut chain = FilterChain::new();
    let cutoff_frequency = target_sample_rate * 0.40;

    chain.add(Box::new(filters::LowPassIIR::new(clock_rate, cutoff_frequency)), clock_rate);

    match filter_type {
        FilterType::Nes => {
            //The NES hardware follows the DACs with a surprisingly involved circuit that adds several low-pass and high-pass filters:

            // A first-order high-pass filter at 90 Hz
            chain.add(Box::new(filters::HighPassIIR::new(target_sample_rate, 90.0)), target_sample_rate);
            //  Another first-order high-pass filter at 440 Hz
            chain.add(Box::new(filters::HighPassIIR::new(target_sample_rate, 440.0)), target_sample_rate);
            // A first-order low-pass filter at 14 kHz
            chain.add(Box::new(filters::LowPassIIR::new(target_sample_rate, 14000.0)), target_sample_rate);
        },
        FilterType::FamiCom => {
            // The Famicom hardware instead ONLY specifies a first-order high-pass filter at 37 Hz, 
            // followed by the unknown (and varying) properties of the RF modulator and demodulator. 
            chain.add(Box::new(filters::HighPassIIR::new(target_sample_rate, 37.0)), target_sample_rate);
        }
    }

    return chain;
}

impl ApuState {
    pub fn new() -> ApuState {
        let default_samplerate = 44100;
        let output_buffer_size = recommended_buffer_size(44100);

        return ApuState {
            current_cycle: 0,
            frame_sequencer_mode: 0,
            frame_sequencer: 0,
            frame_reset_delay: 0,
            quarter_frame_counter: 0,
            half_frame_counter: 0,
            frame_interrupt: false,
            disable_interrupt: false,
            pulse_1: PulseChannelState::new("Pulse 1", "2A03", 1_789_773, true),
            pulse_2: PulseChannelState::new("Pulse 2", "2A03", 1_789_773, false),
            triangle: TriangleChannelState::new("Triangle", "2A03", 1_789_773),
            noise: NoiseChannelState::new("Noise", "2A03"),
            dmc: DmcState::new("DMC", "2A03"),
            staging_buffer: RingBuffer::new(output_buffer_size),
            edge_buffer: RingBuffer::new(output_buffer_size),
            output_buffer: vec!(0i16; output_buffer_size),
            buffer_full: false,
            sample_rate: default_samplerate,
            cpu_clock_rate: 1_789_773,
            generated_samples: 0,
            next_sample_at: 0,
            pulse_table: generate_pulse_table(),
            tnd_table: generate_tnd_table(),

            filter_type: FilterType::FamiCom,
            filter_chain: construct_hq_filter_chain(1789773.0, 44100.0, FilterType::FamiCom),
            filter_hq: true,
        }
    }

    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.staging_buffer = RingBuffer::new(buffer_size);
        self.output_buffer = vec!(0i16; buffer_size);
        self.buffer_full = false;
    }

    pub fn set_sample_rate(&mut self, sample_rate: u64) {
        self.sample_rate = sample_rate;
        self.update_filter();
        let output_buffer_size = recommended_buffer_size(sample_rate);
        self.set_buffer_size(output_buffer_size);
    }

    pub fn set_filter(&mut self, filter_type: FilterType, hq: bool) {
        self.filter_type = filter_type;
        self.filter_hq = hq;
        self.update_filter();
    }

    pub fn update_filter(&mut self) {
        if self.filter_hq {
            self.filter_chain = construct_hq_filter_chain(self.cpu_clock_rate as f32, self.sample_rate as f32, self.filter_type);
        } else {
            self.filter_chain = construct_lq_filter_chain(self.cpu_clock_rate as f32, self.sample_rate as f32, self.filter_type);
        }
    }

    pub fn channels(&self) -> Vec<& dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        channels.push(&self.dmc);
        channels.push(&self.noise);
        channels.push(&self.triangle);
        channels.push(&self.pulse_1);
        channels.push(&self.pulse_2);
        return channels;
    }

    pub fn channels_mut(&mut self) -> Vec<&mut dyn AudioChannelState> {
        let mut channels: Vec<&mut  dyn AudioChannelState> = Vec::new();
        channels.push(&mut self.dmc);
        channels.push(&mut self.noise);
        channels.push(&mut self.triangle);
        channels.push(&mut self.pulse_1);
        channels.push(&mut self.pulse_2);
        return channels;
    }

    pub fn debug_read_register(&self, address: u16) -> u8 {
        match address {
            0x4015 => {
                let mut status = 0;
                if self.pulse_1.length_counter.length > 0 {
                    status += 0b0000_0001;
                }
                if self.pulse_2.length_counter.length > 0 {
                    status += 0b0000_0010;
                }
                if self.triangle.length_counter.length > 0 {
                    status += 0b0000_0100;
                }
                if self.noise.length_counter.length > 0 {
                    status += 0b0000_1000;
                }
                if self.dmc.bytes_remaining > 0 {
                    status += 0b0001_0000;
                }

                if self.frame_interrupt {
                    status += 0b0100_0000;
                }
                if self.dmc.interrupt_flag {
                    status += 0b1000_0000;   
                }
                return status;
            },
            _ => return 0
        }
    }

    pub fn read_register(&mut self, address: u16) -> u8 {
        let data = self.debug_read_register(address);
        match address {
            0x4015 => {
                // Reading from this register resets frame_interrupt:
                self.frame_interrupt = false;                
            },
            _ => {}
        }
        return data;
    }

    pub fn write_register(&mut self, address: u16, data: u8) {
        let duty_table = [
            0b1000_0000,
            0b1100_0000,
            0b1111_0000,
            0b0011_1111,
        ];
        match address {
            // Pulse Channel 1
            0x4000 => {
                let duty_index =      (data & 0b1100_0000) >> 6;
                let length_disable =  (data & 0b0010_0000) != 0;
                let constant_volume = (data & 0b0001_0000) != 0;

                self.pulse_1.duty = duty_table[duty_index as usize];
                self.pulse_1.length_counter.halt_flag = length_disable;
                self.pulse_1.envelope.looping = length_disable;
                self.pulse_1.envelope.enabled = !(constant_volume);
                self.pulse_1.envelope.volume_register = data & 0b0000_1111;
            },
            0x4001 => {
                self.pulse_1.sweep_enabled =  (data & 0b1000_0000) != 0;
                self.pulse_1.sweep_period =   (data & 0b0111_0000) >> 4;
                self.pulse_1.sweep_negate =   (data & 0b0000_1000) != 0;
                self.pulse_1.sweep_shift =     data & 0b0000_0111;
                self.pulse_1.sweep_reload = true;
            },
            0x4002 => {
                let period_low = data as u16;
                self.pulse_1.period_initial = (self.pulse_1.period_initial & 0xFF00) | period_low;
            },
            0x4003 => {
                let period_high =  ((data & 0b0000_0111) as u16) << 8;
                let length_index = (data & 0b1111_1000) >> 3;

                self.pulse_1.period_initial = (self.pulse_1.period_initial & 0x00FF) | period_high;
                self.pulse_1.length_counter.set_length(length_index);

                // Start this note
                self.pulse_1.sequence_counter = 0;
                self.pulse_1.envelope.start_flag = true;
            },

            // Pulse Channel 2
            0x4004 => {
                let duty_index =      (data & 0b1100_0000) >> 6;
                let length_disable =  (data & 0b0010_0000) != 0;
                let constant_volume = (data & 0b0001_0000) != 0;

                self.pulse_2.duty = duty_table[duty_index as usize];
                self.pulse_2.length_counter.halt_flag = length_disable;
                self.pulse_2.envelope.looping = length_disable;
                self.pulse_2.envelope.enabled = !(constant_volume);
                self.pulse_2.envelope.volume_register = data & 0b0000_1111;
            },
            0x4005 => {
                self.pulse_2.sweep_enabled =  (data & 0b1000_0000) != 0;
                self.pulse_2.sweep_period =   (data & 0b0111_0000) >> 4;
                self.pulse_2.sweep_negate =   (data & 0b0000_1000) != 0;
                self.pulse_2.sweep_shift =     data & 0b0000_0111;
                self.pulse_2.sweep_reload = true;
            },
            0x4006 => {
                let period_low = data as u16;
                self.pulse_2.period_initial = (self.pulse_2.period_initial & 0xFF00) | period_low;
            },
            0x4007 => {
                let period_high =  ((data & 0b0000_0111) as u16) << 8;
                let length_index =  (data & 0b1111_1000) >> 3;

                self.pulse_2.period_initial = (self.pulse_2.period_initial & 0x00FF) | period_high;
                self.pulse_2.length_counter.set_length(length_index);

                // Start this note
                self.pulse_2.sequence_counter = 0;
                self.pulse_2.envelope.start_flag = true;
            },

            // Triangle Channel
            0x4008 => {
                self.triangle.control_flag           = (data & 0b1000_0000) != 0;
                self.triangle.length_counter.halt_flag = self.triangle.control_flag;
                self.triangle.linear_counter_initial =  data & 0b0111_1111;
            },
            0x400A => {
                let period_low = data as u16;
                self.triangle.period_initial = (self.triangle.period_initial & 0xFF00) | period_low
            },
            0x400B => {
                let period_high =  ((data & 0b0000_0111) as u16) << 8;
                let length_index =  (data & 0b1111_1000) >> 3;

                self.triangle.period_initial = (self.triangle.period_initial & 0x00FF) | period_high;
                self.triangle.length_counter.set_length(length_index);

                // Start this note
                self.triangle.linear_reload_flag = true;
            },

            // Noise Channel
            0x400C => {
                let length_disable =  (data & 0b0010_0000) != 0;
                let constant_volume = (data & 0b0001_0000) != 0;

                self.noise.length_counter.halt_flag = length_disable;
                self.noise.envelope.looping = length_disable;
                self.noise.envelope.enabled = !(constant_volume);
                self.noise.envelope.volume_register = data & 0b0000_1111;
            },
            0x400E => {
                let noise_period = [
                    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];

                let mode =        (data & 0b1000_0000) >> 7;
                let period_index = data & 0b0000_1111;
                self.noise.mode = mode;
                self.noise.period_initial = noise_period[period_index as usize];
            },
            0x400F => {
                let length_index = (data & 0b1111_1000) >> 3;
                self.noise.length_counter.set_length(length_index);

                // Restart the envelope
                self.noise.envelope.start_flag = true;
            },

            // DMC Channel
            0x4010 => {
                let period_table = [
                    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106,  84,  72,  54];
                self.dmc.looping = (data & 0b0100_0000) != 0;
                self.dmc.interrupt_enabled = (data & 0b1000_0000) != 0;
                if !self.dmc.interrupt_enabled {
                    // If the enable bit was cleared, clear the flag also.
                    self.dmc.interrupt_flag = false;
                }
                let period_index = data & 0b0000_1111;
                self.dmc.period_initial = period_table[period_index as usize] / 2;
            },
            0x4011 => {
                self.dmc.output_level = data & 0b0111_1111;
            },
            0x4012 => {
                self.dmc.starting_address = 0xC000 + (data as u16 * 64);
            },
            0x4013 => {
                self.dmc.sample_length = (data as u16 * 16) + 1;
            },

            // Status / Enabled
            0x4015 => {
                self.pulse_1.length_counter.channel_enabled  = (data & 0b0001) != 0;
                self.pulse_2.length_counter.channel_enabled  = (data & 0b0010) != 0;
                self.triangle.length_counter.channel_enabled = (data & 0b0100) != 0;
                self.noise.length_counter.channel_enabled    = (data & 0b1000) != 0;

                if !(self.pulse_1.length_counter.channel_enabled) {
                    self.pulse_1.length_counter.length = 0;
                }
                if !(self.pulse_2.length_counter.channel_enabled) {
                    self.pulse_2.length_counter.length = 0;
                }
                if !(self.triangle.length_counter.channel_enabled) {
                    self.triangle.length_counter.length = 0;
                }
                if !(self.noise.length_counter.channel_enabled) {
                    self.noise.length_counter.length = 0;
                }

                let dmc_enable = (data & 0b1_0000) != 0;
                if !(dmc_enable) {
                    self.dmc.bytes_remaining = 0;
                }
                if dmc_enable && self.dmc.bytes_remaining == 0 {
                    self.dmc.current_address = self.dmc.starting_address;
                    self.dmc.bytes_remaining = self.dmc.sample_length;
                }
                self.dmc.interrupt_flag = false;
            }

            // Frame Counter / Interrupts
            0x4017 => {
                self.frame_sequencer_mode = (data & 0b1000_0000) >> 7;
                self.disable_interrupt =    (data & 0b0100_0000) != 0;
                if (self.current_cycle & 0b1) != 0 {
                    self.frame_reset_delay = 3;
                } else {
                    self.frame_reset_delay = 4;
                }
                // If interrupts are disabled, clear the flag too:
                if self.disable_interrupt {
                    self.frame_interrupt = false;
                }
            }

            _ => ()
        }
    }

    // Note: this uses CPU clocks, NOT APU clocks! It's simpler to represent the half-clock
    // updates this way. Documentation: https://wiki.nesdev.com/w/index.php/APU_Frame_Counter

    pub fn clock_frame_sequencer(&mut self) {
        if self.frame_reset_delay > 0 {
            self.frame_reset_delay -= 1;
            if self.frame_reset_delay == 0 {
                self.frame_sequencer = 0;
                if self.frame_sequencer_mode == 1 {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
            }
        }

        if self.frame_sequencer_mode == 0 {
            // 4-step sequence
            match self.frame_sequencer {
                7457 => self.clock_quarter_frame(),
                14913 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                },
                22371 => self.clock_quarter_frame(),
                29828 => {
                    if !self.disable_interrupt {
                        self.frame_interrupt = true;
                    }
                },
                29829 => {
                    if !self.disable_interrupt {
                        self.frame_interrupt = true;
                    }
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                },
                29830 => {
                    if !self.disable_interrupt {
                        self.frame_interrupt = true;
                    }
                    self.frame_sequencer = 0;
                },
                _ => ()
            }
        } else {
            match self.frame_sequencer {
                // "5-step" sequence (uneven timing)
                7457 => self.clock_quarter_frame(),
                14913 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                },
                22371 => self.clock_quarter_frame(),
                37281 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                },
                37282 => {
                  self.frame_sequencer = 0;  
                },
                _ => ()
            }
        }
        
        self.frame_sequencer += 1;
    }

    pub fn clock_quarter_frame(&mut self) {
        self.pulse_1.envelope.clock();
        self.pulse_2.envelope.clock();
        self.triangle.update_linear_counter();
        self.noise.envelope.clock();
        self.quarter_frame_counter += 1;
    }

    pub fn clock_half_frame(&mut self) {
        self.pulse_1.update_sweep();
        self.pulse_2.update_sweep();

        self.pulse_1.length_counter.clock();
        self.pulse_2.length_counter.clock();
        self.triangle.length_counter.clock();
        self.noise.length_counter.clock();
        self.half_frame_counter += 1;
    }

    pub fn clock_apu(&mut self, mapper: &mut dyn Mapper) {
        self.clock_frame_sequencer();

        // Clock the triangle channel once per CPU cycle
        self.triangle.clock();
        self.noise.clock();

        // Only clock Pulse channels on every other cycle
        // (Most documentation calls this once per APU cycle)
        if (self.current_cycle & 0b1) == 0 {
            self.pulse_1.clock();
            self.pulse_2.clock();
            self.dmc.clock(mapper);
        }
        
        // Collect current samples from the various channels
        let pulse_1_sample = self.pulse_1.output();
        let pulse_2_sample = self.pulse_2.output();
        let triangle_sample = self.triangle.output();
        let noise_sample = self.noise.output();
        let dmc_sample = self.dmc.output();

        // Mix samples, using the LUT we generated earlier, based on documentation here:
        // https://wiki.nesdev.com/w/index.php/APU_Mixer
        let mut combined_pulse = 0;
        if !(self.pulse_1.debug_disable) {
            combined_pulse += pulse_1_sample;
        }
        if !(self.pulse_2.debug_disable) {
            combined_pulse += pulse_2_sample;
        }
        let pulse_output = self.pulse_table[combined_pulse as usize];
        
        let tri_output = if self.triangle.debug_disable {0} else {triangle_sample};
        let noise_output = if self.noise.debug_disable {0} else {noise_sample};
        let dmc_output = if self.dmc.debug_disable {0} else {dmc_sample};
        let tnd_output = self.tnd_table[full_tnd_index(tri_output as usize, noise_output as usize, dmc_output as usize)];

        let current_2a03_sample = (pulse_output - 0.5) + (tnd_output - 0.5);
        let current_dac_sample = mapper.mix_expansion_audio(current_2a03_sample) as f32;

        // apply filters NEW
        self.filter_chain.consume(current_dac_sample, 1.0 / (self.cpu_clock_rate as f32));

        if self.current_cycle >= self.next_sample_at { 
            // decimate sample
            let composite_sample = (self.filter_chain.output() * 32767.0) as i16;

            self.staging_buffer.push(composite_sample);
            self.edge_buffer.push(true as i16);

            // Write debug buffers from these, regardless of enable / disable status
            self.pulse_1.record_current_output();
            self.pulse_2.record_current_output();
            self.triangle.record_current_output();
            self.noise.record_current_output();
            self.dmc.record_current_output();
            mapper.record_expansion_audio_output(current_2a03_sample);

            self.generated_samples += 1;
            self.next_sample_at = ((self.generated_samples + 1) * self.cpu_clock_rate) / self.sample_rate;

            if self.staging_buffer.index() == 0 {
                self.output_buffer.copy_from_slice(self.staging_buffer.buffer());
                self.buffer_full = true;
            }
        }

        self.current_cycle += 1;
    }

    pub fn samples_queued(&self) -> usize {
        let mut sample_count = self.staging_buffer.index();
        if self.buffer_full {
            sample_count += self.output_buffer.len();
        }
        return sample_count;
    }

    pub fn dump_sample_buffer(&self) {
        let mut file =
            OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("audiodump.raw")
            .unwrap();

        // turn our sample buffer into a simple file buffer for output
        let mut buffer = [0u8; 1024 * 2];
        for i in 0 .. 1024 {
            let sample = self.output_buffer[i];
            buffer[i * 2]     = (((sample as u16) & 0xFF00) >> 8) as u8;
            buffer[i * 2 + 1] = (((sample as u16) & 0x00FF)     ) as u8;
        }

        let _ = file.write_all(&buffer);
    }

    pub fn consume_samples(&mut self) -> Vec<i16> {
        let mut output_buffer = vec!(0i16; 0);
        if self.buffer_full {
            output_buffer.extend(&self.output_buffer);
            self.buffer_full = false;
        }
        let staging_index = self.staging_buffer.index();
        output_buffer.extend(&self.staging_buffer.buffer()[0 .. staging_index]);
        self.staging_buffer.reset();
        return output_buffer;
    }

    pub fn irq_signal(&self) -> bool {
        return self.frame_interrupt || self.dmc.interrupt_flag;
    }

    pub fn mute_channel(&mut self, mapper: &mut dyn Mapper, channel_index: usize) {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        channels.extend(self.channels_mut());
        channels.extend(mapper.channels_mut());
        if channel_index < channels.len() {
            channels[channel_index].mute();
        }
    }

    pub fn unmute_channel(&mut self, mapper: &mut dyn Mapper, channel_index: usize) {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        channels.extend(self.channels_mut());
        channels.extend(mapper.channels_mut());
        if channel_index < channels.len() {
            channels[channel_index].unmute();
        }
    }
}

// The APU itself counts as a channel, loosely, mostly for debugging purposes. Its output is a
// simple waveform, and it provides no useful frequency information.
impl AudioChannelState for ApuState {
    fn name(&self) -> String {
        return "Final Mix".to_string();
    }

    fn chip(&self) -> String {
        return "APU".to_string();
    }

    fn edge_buffer(&self) -> &RingBuffer {
        return &self.edge_buffer;
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.staging_buffer;
    }

    fn record_current_output(&mut self) {
    }

    fn min_sample(&self) -> i16 {
        return -16384;
    }

    fn max_sample(&self) -> i16 {
        return 16383;
    }

    fn muted(&self) -> bool {
        return false;
    }

    fn mute(&mut self) {
    }

    fn unmute(&mut self) {        
    }
}

