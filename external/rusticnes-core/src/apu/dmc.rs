use mmc::mapper::Mapper;
use super::audio_channel::AudioChannelState;
use super::ring_buffer::RingBuffer;
use super::filters;
use super::filters::DspFilter;

pub struct DmcState {
    pub name: String,
    pub chip: String,
    pub debug_disable: bool,
    pub output_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub last_edge: bool,
    pub debug_filter: filters::HighPassIIR,

    pub looping: bool,
    pub period_initial: u16,
    pub period_current: u16,
    pub output_level: u8,
    pub starting_address: u16,
    pub sample_length: u16,

    pub current_address: u16,
    pub sample_buffer: u8,
    pub shift_register: u8,
    pub sample_buffer_empty: bool,
    pub bits_remaining: u8,
    pub bytes_remaining: u16,
    pub silence_flag: bool,

    pub interrupt_enabled: bool,
    pub interrupt_flag: bool,
    pub rdy_line: bool,
    pub rdy_delay: u8,
}

impl DmcState {
    pub fn new(channel_name: &str, chip_name: &str) -> DmcState {
        return DmcState {
            name: String::from(channel_name),
            chip: String::from(chip_name),
            debug_disable: false,
            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0),

            looping: false,
            period_initial: 428,
            period_current: 0,
            output_level: 0,
            starting_address: 0,
            sample_length: 0,

            current_address: 0,
            sample_buffer: 0,
            shift_register: 0,
            sample_buffer_empty: true,
            bits_remaining: 8,
            bytes_remaining: 0,
            silence_flag: false,
            interrupt_enabled: true,
            interrupt_flag: false,
            rdy_line: false,
            rdy_delay: 0,
        }
    }

    pub fn debug_status(&self) -> String {
        return format!("Rate: {:3} - Divisor: {:3} - Start: {:04X} - Current: {:04X} - Length: {:4} - R.Bytes: {:4} - R.Bits: {:1}", 
            self.period_initial, self.period_current, self.starting_address, self.current_address, self.sample_length,
            self.bytes_remaining, self.bits_remaining);
    }

    pub fn read_next_sample(&mut self, mapper: &mut dyn Mapper) {
        match mapper.read_cpu(0x8000 | (self.current_address & 0x7FFF)) {
            Some(byte) => self.sample_buffer = byte,
            None => self.sample_buffer = 0,
        }
        self.current_address = self.current_address.wrapping_add(1);
        self.bytes_remaining -= 1;
        if self.bytes_remaining == 0 {
            if self.looping {
                self.current_address = self.starting_address;
                self.bytes_remaining = self.sample_length;
                self.last_edge = true;
            } else {
                if self.interrupt_enabled {
                    self.interrupt_flag = true;
                }
            }
        }
        self.sample_buffer_empty = false;
        self.rdy_line = false;
        self.rdy_delay = 0;
    }

    pub fn begin_output_cycle(&mut self) {
        self.bits_remaining = 8;
        if self.sample_buffer_empty {
            self.silence_flag = true;
        } else {
            self.silence_flag = false;
            self.shift_register = self.sample_buffer;
            self.sample_buffer_empty = true;
        }
    }

    pub fn update_output_unit(&mut self) {
        if !(self.silence_flag) {
            let mut target_output = self.output_level;
            if (self.shift_register & 0b1) == 0 {
                if self.output_level >= 2 {
                    target_output -= 2;
                }
            } else  {
                if self.output_level <= 125 {
                    target_output += 2;
                }
            }
            self.output_level = target_output;
        }
        self.shift_register = self.shift_register >> 1;
        self.bits_remaining -= 1;
        if self.bits_remaining == 0 {
            self.begin_output_cycle();
        }
    }

    pub fn clock(&mut self, mapper: &mut dyn Mapper) {
        if self.period_current == 0 {
            self.period_current = self.period_initial - 1;
            self.update_output_unit();
        } else {
            self.period_current -= 1;
        }
        if self.sample_buffer_empty && self.bytes_remaining > 0 {
            self.rdy_line = true;
            self.rdy_delay += 1;
            if self.rdy_delay > 2 {
                self.read_next_sample(mapper);
            }
        } else {
            self.rdy_line = false;
            self.rdy_delay = 0;
        }
    }

    pub fn output(&self) -> i16 {
        return self.output_level as i16;
    }
}

impl AudioChannelState for DmcState {
    fn name(&self) -> String {
        return self.name.clone();
    }

    fn chip(&self) -> String {
        return self.chip.clone();
    }

    fn edge_buffer(&self) -> &RingBuffer {
        return &self.edge_buffer;
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.output_buffer;
    }

    fn record_current_output(&mut self) {
        self.debug_filter.consume(self.output() as f32);
        self.output_buffer.push((self.debug_filter.output() * -4.0) as i16);
        self.edge_buffer.push(self.last_edge as i16);
        self.last_edge = false;
    }

    fn min_sample(&self) -> i16 {
        return -512;
    }

    fn max_sample(&self) -> i16 {
        return 512;
    }

    fn muted(&self) -> bool {
        return self.debug_disable;
    }

    fn mute(&mut self) {
        self.debug_disable = true;
    }

    fn unmute(&mut self) {
        self.debug_disable = false;
    }

    fn playing(&self) -> bool {
        return self.amplitude() > 0.0;
    }

    fn amplitude(&self) -> f32 {
        let buffer = self.output_buffer.buffer();
        let mut index = (self.output_buffer.index() - 256) % buffer.len();
        let mut max = buffer[index];
        let mut min = buffer[index];
        for _i in 0 .. 256 {
            if buffer[index] > max {max = buffer[index];}
            if buffer[index] < min {min = buffer[index];}
            index += 1;
            index = index % buffer.len();
        }
        return (max - min) as f32 / 256.0;
    }
}