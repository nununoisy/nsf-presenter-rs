use super::length_counter::LengthCounterState;
use super::volume_envelope::VolumeEnvelopeState;
use super::audio_channel::AudioChannelState;
use super::audio_channel::PlaybackRate;
use super::audio_channel::Volume;
use super::audio_channel::Timbre;
use super::ring_buffer::RingBuffer;
use super::filters;
use super::filters::DspFilter;

pub struct NoiseChannelState {
    pub name: String,
    pub chip: String,
    pub debug_disable: bool,
    pub output_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub last_edge: bool,
    pub debug_filter: filters::HighPassIIR,
    pub length: u8,
    pub length_halt_flag: bool,

    pub envelope: VolumeEnvelopeState,
    pub length_counter: LengthCounterState,

    pub mode: u8,
    pub period_initial: u16,
    pub period_current: u16,

    // Actually a 15-bit register
    pub shift_register: u16,
}

impl NoiseChannelState {
    pub fn new(channel_name: &str, chip_name: &str, ) -> NoiseChannelState {
        return NoiseChannelState {
            name: String::from(channel_name),
            chip: String::from(chip_name),
            debug_disable: false,
            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0),
            length: 0,
            length_halt_flag: false,

            envelope: VolumeEnvelopeState::new(),
            length_counter: LengthCounterState::new(),
            mode: 0,
            period_initial: 0,
            period_current: 0,

            // Actually a 15-bit register
            shift_register: 1,
        }
    }

    pub fn clock(&mut self) {
        if self.period_current == 0 {
            self.period_current = self.period_initial - 1;

            let mut feedback = self.shift_register & 0b1;
            if self.mode == 1 {
                feedback ^= (self.shift_register >> 6) & 0b1;
            } else {
                feedback ^= (self.shift_register >> 1) & 0b1;
            }
            self.shift_register = self.shift_register >> 1;
            self.shift_register |= feedback << 14;
            self.last_edge = true;
        } else {
            self.period_current -= 1;
        }
    }

    pub fn output(&self) -> i16 {
        if self.length_counter.length > 0 {
            let mut sample = (self.shift_register & 0b1) as i16;
            sample *= self.envelope.current_volume() as i16;
            return sample;
        } else {
            return 0;
        }
    }
}

impl AudioChannelState for NoiseChannelState {
    fn name(&self) -> String {
        return self.name.clone();
    }

    fn chip(&self) -> String {
        return self.chip.clone();
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.output_buffer;
    }

    fn edge_buffer(&self) -> &RingBuffer {
        return &self.edge_buffer;
    }

    fn record_current_output(&mut self) {
        self.debug_filter.consume(self.output() as f32);
        self.output_buffer.push((self.debug_filter.output() * -4.0) as i16);
        self.edge_buffer.push(self.last_edge as i16);
        self.last_edge = false;
    }

    fn min_sample(&self) -> i16 {
        return -60;
    }

    fn max_sample(&self) -> i16 {
        return 60;
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
        return 
            (self.length_counter.length > 0) &&
            (self.envelope.current_volume() > 0);
    }

    fn rate(&self) -> PlaybackRate {
        let lsfr_index = match self.period_initial {
            4    => {0xF},
            8    => {0xE},
            16   => {0xD},
            32   => {0xC},
            64   => {0xB},
            96   => {0xA},
            128  => {0x9},
            160  => {0x8},
            202  => {0x7},
            254  => {0x6},
            380  => {0x5},
            508  => {0x4},
            762  => {0x3},
            1016 => {0x2},
            2034 => {0x1},
            4068 => {0x0},
            _ => {0x0} // also unreachable
        };
        return PlaybackRate::LfsrRate {index: lsfr_index, max: 0xF};
    }

    fn volume(&self) -> Option<Volume> {
        return Some(Volume::VolumeIndex{ index: self.envelope.current_volume() as usize, max: 15 });
    }

    fn timbre(&self) -> Option<Timbre> {
        return Some(Timbre::LsfrMode{index: self.mode as usize, max: 1});
    }
}