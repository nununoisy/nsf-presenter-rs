// https://www.nesdev.org/wiki/VRC7
// https://www.nesdev.org/wiki/VRC7_audio

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

use apu::AudioChannelState;
use apu::PlaybackRate;
use apu::Volume;
use apu::Timbre;
use apu::RingBuffer;
use apu::filters;
use apu::filters::DspFilter;

pub struct Vrc7 {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,

    pub mirroring: Mirroring,
    pub vram: Vec<u8>,

    pub chr_banks: Vec<u8>,
    pub prg_banks: Vec<u8>,
    pub submapper: u8,

    pub irq_scanline_prescaler: i16,
    pub irq_latch: u8,
    pub irq_scanline_mode: bool,
    pub irq_enable: bool,
    pub irq_enable_after_acknowledgement: bool,
    pub irq_pending: bool,
    pub irq_counter: u8,

    pub audio_register: u8,

    pub audio: Vrc7Audio,
}

impl Vrc7 {
    pub fn from_ines(ines: INesCartridge) -> Result<Vrc7, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Vrc7 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            mirroring: ines.header.mirroring(),
            vram: vec![0u8; 0x1000],
            chr_banks: vec![0u8; 8],
            prg_banks: vec![0u8; 3],
            submapper: ines.header.submapper_number(),

            irq_scanline_prescaler: 0,
            irq_latch: 0,
            irq_scanline_mode: false,
            irq_enable: false,
            irq_enable_after_acknowledgement: false,
            irq_pending: false,
            irq_counter: 0,

            audio: Vrc7Audio::new(),
            audio_register: 0,
        });
    }

    fn _clock_irq_prescaler(&mut self) {
        self.irq_scanline_prescaler -= 3;
        if self.irq_scanline_prescaler <= 0 {
            self._clock_irq_counter();
            self.irq_scanline_prescaler += 341;
        }
    }

    fn _clock_irq_counter(&mut self) {
        if self.irq_counter == 0xFF {
            self.irq_counter = self.irq_latch;
            self.irq_pending = true;
        } else {
            self.irq_counter += 1;
        }
    }
}

pub fn amplitude_from_db(db: f32) -> f32 {
    return f32::powf(10.0, db / 20.0);
}

impl Mapper for Vrc7 {
    fn print_debug_status(&self) {
        println!("======= VRC7 =======");
        println!("Mirroring Mode: {}", mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn clock_cpu(&mut self) {
        if self.irq_enable {
            if self.irq_scanline_mode {
                self._clock_irq_prescaler();
            } else {
                self._clock_irq_counter();
            }
        }
        self.audio.clock();
    }

    fn mix_expansion_audio(&self, nes_sample: f32) -> f32 {
        let combined_vrc7_audio = self.audio.output() as f32
            / 256.0 // to go from +256/-256 to +1/-1
            / 6.0;  // number of vrc7 channels

        // I measured the above mix with the db_vrc7.nes test from rainwarrior's
        // audio survey, found here https://forums.nesdev.org/viewtopic.php?t=17741
        // and found that the VRC7 is 6.23 dB louder than the APU.

        // The NSFe defaults the VRC7 to +11 dB relative to the APU:
        // https://www.nesdev.org/wiki/NSFe#mixe
        // This also aligns neatly with several tests in that forum thread, so it's what
        // I'll run with here.
        let stock_vrc7_db = 6.23;
        let desired_vrc7_db = 11.00;
        let mixed_vrc7_audio = combined_vrc7_audio * amplitude_from_db(desired_vrc7_db - stock_vrc7_db);

        return mixed_vrc7_audio + nes_sample;
    }

    fn irq_flag(&self) -> bool {
        return self.irq_pending;
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x6000 ..= 0x7FFF => {self.prg_ram.wrapping_read((address - 0x6000) as usize)},
            0x8000 ..= 0x9FFF => self.prg_rom.banked_read(0x2000, self.prg_banks[0] as usize, address as usize),
            0xA000 ..= 0xBFFF => self.prg_rom.banked_read(0x2000, self.prg_banks[1] as usize, address as usize),
            0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, self.prg_banks[2] as usize, address as usize),
            0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF, address as usize),
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x6000 ..= 0x7FFF => {self.prg_ram.wrapping_write((address - 0x6000) as usize, data);},
            0x8000 ..= 0xFFFF => {
                let register_mask = match self.submapper {
                    1 => 0xF028,
                    2 => 0xF030,
                    _ => 0xF030
                };
                let register_address = address & register_mask;
                match register_address {
                    0x8000          => {self.prg_banks[0] = data & 0b0011_1111},
                    0x8010 | 0x8008 => {self.prg_banks[1] = data & 0b0011_1111},
                    0x9000          => {self.prg_banks[2] = data & 0b0011_1111},
                    0xA000          => {self.chr_banks[0] = data},
                    0xA008 | 0xA010 => {self.chr_banks[1] = data},
                    0xB000          => {self.chr_banks[2] = data},
                    0xB008 | 0xB010 => {self.chr_banks[3] = data},
                    0xC000          => {self.chr_banks[4] = data},
                    0xC008 | 0xC010 => {self.chr_banks[5] = data},
                    0xD000          => {self.chr_banks[6] = data},
                    0xD008 | 0xD010 => {self.chr_banks[7] = data},
                    0x9010          => {
                        self.audio_register = data
                    },
                    0x9030          => {
                        self.audio.write(self.audio_register, data);
                    },
                    0xE000         => {
                        match data & 0b0000_0011 {
                            0 => self.mirroring = Mirroring::Vertical,
                            1 => self.mirroring = Mirroring::Horizontal,
                            2 => self.mirroring = Mirroring::OneScreenLower,
                            3 => self.mirroring = Mirroring::OneScreenUpper,
                            _ => {}
                        }
                        // for now, ignoring both WRAM protect and sound reset
                    },
                    0xE008 | 0xE010 => { self.irq_latch = data; },
                    0xF000         => {
                        self.irq_scanline_mode = ((data & 0b0000_0100) >> 2) == 0;
                        self.irq_enable = (data & 0b0000_0010) != 0;
                        self.irq_enable_after_acknowledgement = (data & 0b0000_0001) != 0;

                        // acknowledge the pending IRQ if there is one
                        self.irq_pending = false;

                        // If the enable bit is set, setup for the next IRQ immediately, otherwise
                        // do nothing (we may already have one in flight)
                        if self.irq_enable {
                            self.irq_counter = self.irq_latch;
                            self.irq_scanline_prescaler = 344; // we are also clocked on this cycle, so +3 here to undo that for timing
                        }

                    },
                    0xF008 | 0xF010 => {
                        self.irq_pending = false;
                        self.irq_enable = self.irq_enable_after_acknowledgement;
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x03FF => {self.chr.banked_read(0x400, self.chr_banks[0] as usize, address as usize)},
            0x0400 ..= 0x07FF => {self.chr.banked_read(0x400, self.chr_banks[1] as usize, address as usize)},
            0x0800 ..= 0x0BFF => {self.chr.banked_read(0x400, self.chr_banks[2] as usize, address as usize)},
            0x0C00 ..= 0x0FFF => {self.chr.banked_read(0x400, self.chr_banks[3] as usize, address as usize)},
            0x1000 ..= 0x13FF => {self.chr.banked_read(0x400, self.chr_banks[4] as usize, address as usize)},
            0x1400 ..= 0x17FF => {self.chr.banked_read(0x400, self.chr_banks[5] as usize, address as usize)},
            0x1800 ..= 0x1BFF => {self.chr.banked_read(0x400, self.chr_banks[6] as usize, address as usize)},
            0x1C00 ..= 0x1FFF => {self.chr.banked_read(0x400, self.chr_banks[7] as usize, address as usize)},
            0x2000 ..= 0x3FFF => return match self.mirroring {
                Mirroring::Horizontal => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                Mirroring::Vertical   => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                Mirroring::OneScreenLower => Some(self.vram[mirroring::one_screen_lower(address) as usize]),
                Mirroring::OneScreenUpper => Some(self.vram[mirroring::one_screen_upper(address) as usize]),
                _ => None
            },
            _ => return None
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        match address {
            0x0000 ..= 0x03FF => {self.chr.banked_write(0x400, self.chr_banks[0] as usize, address as usize, data)},
            0x0400 ..= 0x07FF => {self.chr.banked_write(0x400, self.chr_banks[1] as usize, address as usize, data)},
            0x0800 ..= 0x0BFF => {self.chr.banked_write(0x400, self.chr_banks[2] as usize, address as usize, data)},
            0x0C00 ..= 0x0FFF => {self.chr.banked_write(0x400, self.chr_banks[3] as usize, address as usize, data)},
            0x1000 ..= 0x13FF => {self.chr.banked_write(0x400, self.chr_banks[4] as usize, address as usize, data)},
            0x1400 ..= 0x17FF => {self.chr.banked_write(0x400, self.chr_banks[5] as usize, address as usize, data)},
            0x1800 ..= 0x1BFF => {self.chr.banked_write(0x400, self.chr_banks[6] as usize, address as usize, data)},
            0x1C00 ..= 0x1FFF => {self.chr.banked_write(0x400, self.chr_banks[7] as usize, address as usize, data)},
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                Mirroring::OneScreenLower => self.vram[mirroring::one_screen_lower(address) as usize] = data,
                Mirroring::OneScreenUpper => self.vram[mirroring::one_screen_upper(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }

    fn has_sram(&self) -> bool {
        return true;
    }

    fn get_sram(&self) -> Vec<u8> {
        return self.prg_ram.as_vec().clone();
    }

    fn load_sram(&mut self, sram_data: Vec<u8>) {
        *self.prg_ram.as_mut_vec() = sram_data;
    }

    fn channels(&self) ->  Vec<& dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        channels.push(&self.audio.channel1);
        channels.push(&self.audio.channel2);
        channels.push(&self.audio.channel3);
        channels.push(&self.audio.channel4);
        channels.push(&self.audio.channel5);
        channels.push(&self.audio.channel6);
        return channels;
    }

    fn channels_mut(&mut self) ->  Vec<&mut dyn AudioChannelState> {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        channels.push(&mut self.audio.channel1);
        channels.push(&mut self.audio.channel2);
        channels.push(&mut self.audio.channel3);
        channels.push(&mut self.audio.channel4);
        channels.push(&mut self.audio.channel5);
        channels.push(&mut self.audio.channel6);
        return channels;
    }

    fn record_expansion_audio_output(&mut self, _nes_sample: f32) {
        self.audio.record_output();
    }
}

// TODO: explore and see if we can't somehow make these constant while keeping them
// in function form. (We ideally do not want to store the full baked table in source)
fn generate_logsin_lut() -> Vec<u16> {
    let mut logsin_lut = vec!(0u16; 256);
    for n in 0 ..= 255 {
        let i = n as f32 + 0.5;
        let x = i * (std::f32::consts::PI / 2.0) / 256.0;
        logsin_lut[n] = (f32::log2(f32::sin(x)) * -256.0).round() as u16;
    }
    return logsin_lut;
}

fn generate_exp_table() -> Vec<u16> {
    let mut exp_lut = vec!(0u16; 256);
    for n in 0 ..= 255 {
        let i = n as f32 / 256.0;
        exp_lut[n] = ((f32::exp2(i) * 1024.0) - 1024.0).round() as u16
    }
    return exp_lut;
}

pub const KSL_INIT: [i16; 16] = [
    0,  48,  64,  74,  80,  86,  90,  94,
    96, 100, 102, 104, 106, 108, 110, 112
];

fn generate_ksl_lut() -> Vec<u16> {
    let mut ksl_lut = vec!(0u16; 4 * 8 * 16);
    for ksl in 0 .. 4 {
        for octave in 0 .. 8 {
            for fnum in 0 .. 16 {
                let adjusted_attenuation = KSL_INIT[fnum] - 16 * (7 - octave);
                let effective_attenuation = if (adjusted_attenuation <= 0) || (ksl == 0) {
                    0
                } else {
                    adjusted_attenuation >> (3 - ksl)
                };
                ksl_lut[(ksl as usize * 8 * 16) + (octave as usize * 16) + fnum] = effective_attenuation as u16;
            }
        }
    }
    return ksl_lut;
}

fn generate_am_lut() -> Vec<u16> {
    let mut am_lut = Vec::new();
    for i in 0 .. 14 {
        match i {
            0 => am_lut.append(&mut vec!(i; 11)),
            13 => am_lut.append(&mut vec!(i; 3)),
            _ => am_lut.append(&mut vec!(i; 8)),
        }
    }
    for i in (0 .. 13).rev() {
        match i {
            0 => am_lut.append(&mut vec!(i; 4)),
            _ => am_lut.append(&mut vec!(i; 8)),
        }
    }
    return am_lut;
}

pub const FM_LFO_LUT: [i16; 8 * 8] = [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 1, 0, 0, 0,-1, 0,
    0, 1, 2, 1, 0,-1,-2,-1,
    0, 1, 3, 1, 0,-1,-3,-1,
    0, 2, 4, 2, 0,-2,-4,-2,
    0, 2, 5, 2, 0,-2,-5,-2,
    0, 3, 6, 3, 0,-3,-6,-3,
    0, 3, 7, 3, 0,-3,-7,-3,
];

pub const DEFAULT_PATCH_TABLE: [u8; 8 * 15] = [
    0x03, 0x21, 0x05, 0x06, 0xE8, 0x81, 0x42, 0x27, // Buzzy Bell
    0x13, 0x41, 0x14, 0x0D, 0xD8, 0xF6, 0x23, 0x12, // Guitar
    0x11, 0x11, 0x08, 0x08, 0xFA, 0xB2, 0x20, 0x12, // Wurly
    0x31, 0x61, 0x0C, 0x07, 0xA8, 0x64, 0x61, 0x27, // Flute
    0x32, 0x21, 0x1E, 0x06, 0xE1, 0x76, 0x01, 0x28, // Clarinet
    0x02, 0x01, 0x06, 0x00, 0xA3, 0xE2, 0xF4, 0xF4, // Synth
    0x21, 0x61, 0x1D, 0x07, 0x82, 0x81, 0x11, 0x07, // Trumpet
    0x23, 0x21, 0x22, 0x17, 0xA2, 0x72, 0x01, 0x17, // Organ
    0x35, 0x11, 0x25, 0x00, 0x40, 0x73, 0x72, 0x01, // Bells
    0xB5, 0x01, 0x0F, 0x0F, 0xA8, 0xA5, 0x51, 0x02, // Vibes
    0x17, 0xC1, 0x24, 0x07, 0xF8, 0xF8, 0x22, 0x12, // Vibraphone
    0x71, 0x23, 0x11, 0x06, 0x65, 0x74, 0x18, 0x16, // Tutti
    0x01, 0x02, 0xD3, 0x05, 0xC9, 0x95, 0x03, 0x02, // Fretless
    0x61, 0x63, 0x0C, 0x00, 0x94, 0xC0, 0x33, 0xF6, // Synth Bass
    0x21, 0x72, 0x0D, 0x00, 0xC1, 0xD5, 0x56, 0x06  // Sweep
];

pub const MT_LUT: [u32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

pub const ADSR_RATE_LUT: [u8; 32] = [
    0, 1, 0, 1, 0, 1, 0, 1, // 4 out of 8
    0, 1, 0, 1, 1, 1, 0, 1, // 5 out of 8
    0, 1, 1, 1, 0, 1, 1, 1, // 6 out of 8
    0, 1, 1, 1, 1, 1, 1, 1  // 7 out of 8
];

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum EnvState {
    Damp,
    Attack,
    Decay,
    Sustain
}

pub struct Vrc7AudioChannel {
    logsin_lut: Vec<u16>,
    exp_lut: Vec<u16>,
    ksl_lut: Vec<u16>,
    am_lut: Vec<u16>,

    fnum: u32,
    octave: u32,
    volume: u16,
    instrument_index: u8,

    carrier_phase: u32,
    modulator_phase: u32,

    // Register $00
    modulator_tremolo: bool,
    modulator_vibrato: bool,
    modulator_sustain_enabled: bool,
    modulator_key_scaling: bool,
    modulator_multiplier: usize,

    // Register $01
    carrier_tremolo: bool,
    carrier_vibrato: bool,
    carrier_sustain_enabled: bool,
    carrier_key_scaling: bool,
    carrier_multiplier: usize,

    // Register $02
    modulator_key_level_scaling: usize,
    modulator_output_level: u16,

    // Register $03
    carrier_key_level_scaling: usize,
    carrier_rectified: bool,
    modulator_rectified: bool,
    feedback: u8,

    // Register $04
    modulator_attack_rate: u8,
    modulator_decay_rate: u8,

    // Reigster $05
    carrier_attack_rate: u8,
    carrier_decay_rate: u8,

    // Register $06
    modulator_sustain_level: u8,
    modulator_release_rate: u8,

    // Register $07
    carrier_sustain_level: u8,
    carrier_release_rate: u8,

    // Internal State
    global_counter: u32,
    carrier_env_level: u8,
    carrier_env_state: EnvState,
    modulator_env_level: u8,
    modulator_env_state: EnvState,
    modulator_previous_0: i16,
    modulator_previous_1: i16,

    key_on: bool,
    sustain_mode: bool,
    channel_index: u8,

    output_buffer: RingBuffer,
    edge_buffer: RingBuffer,
    last_edge: bool,
    debug_filter: filters::HighPassIIR,
    debug_disable: bool,
    am_pos: usize,
    am_counter: u8,
    fm_pos: usize,
    fm_counter: u16,

    current_output: i16,
}

impl Vrc7AudioChannel {
    pub fn new(channel_index: u8) -> Vrc7AudioChannel {
        return Vrc7AudioChannel {
            logsin_lut: generate_logsin_lut(),
            exp_lut: generate_exp_table(),
            ksl_lut: generate_ksl_lut(),
            am_lut: generate_am_lut(),

            fnum: 0,
            octave: 0,
            volume: 0,
            instrument_index: 0,

            carrier_phase: 0,
            modulator_phase: 0,

            modulator_tremolo: false,
            modulator_vibrato: false,
            modulator_sustain_enabled: false,
            modulator_key_scaling: false,
            modulator_multiplier: 0,

            carrier_tremolo: false,
            carrier_vibrato: false,
            carrier_sustain_enabled: false,
            carrier_key_scaling: false,
            carrier_multiplier: 0,

            modulator_key_level_scaling: 0,
            modulator_output_level: 0,

            // Register $03
            carrier_key_level_scaling: 0,
            carrier_rectified: true,
            modulator_rectified: true,
            feedback: 0,

            // Register $04
            modulator_attack_rate: 0,
            modulator_decay_rate: 0,

            // Reigster $05
            carrier_attack_rate: 0,
            carrier_decay_rate: 0,

            // Register $06
            modulator_sustain_level: 0,
            modulator_release_rate: 0,

            // Register $07
            carrier_sustain_level: 0,
            carrier_release_rate: 0,

            // Internal state
            global_counter: 0,

            carrier_env_level: 127,
            carrier_env_state: EnvState::Sustain,
            modulator_env_level: 127,
            modulator_env_state: EnvState::Sustain,

            modulator_previous_0: 0,
            modulator_previous_1: 0,

            key_on: false,
            sustain_mode: false,
            channel_index: channel_index,

            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0),
            debug_disable: false,

            am_pos: 0,
            am_counter: 0,
            fm_pos: 0,
            fm_counter: 0,

            current_output: 0,
        };
    }

    pub fn lookup_logsin(&self, i: usize, wave_rectification: bool) -> u16 {
        let quadrant = (i & 0x300) >> 8;
        let index = i & 0xFF;
        match  quadrant {
            0 => self.logsin_lut[index],
            1 => self.logsin_lut[255 - index],
            2 => {
                if wave_rectification {
                    self.logsin_lut[0]
                } else {
                    0x8000 | self.logsin_lut[index]
                }
            },
            3 => {
                if wave_rectification {
                    self.logsin_lut[0]
                } else {
                    0x8000 | self.logsin_lut[255 - index]
                }
            },
            _ => {0} // should be unreachable
        }
    }

    pub fn lookup_exp(&self, i: u16) -> i16 {
        let sign = i & 0x8000;
        let integral_magnitude =    (i & 0x7F00) >> 8;
        let fractional_magnitude =   i & 0x00FF;
        // Note: we might shift by 16 or more, so we need to expand to i32, as otherwise the
        // result is undefined
        let t_value = ((self.exp_lut[(255 - fractional_magnitude) as usize] + 1024) << 1) as i32;
        let positive_result = (t_value >> integral_magnitude) as i16;
        let signed_result = if sign != 0 {
            !positive_result
        } else {
            positive_result
        };

        return signed_result;
    }

    pub fn clock_global_counter(&mut self) {
        self.global_counter += 1;
    }

    pub fn load_patch(&mut self, patch: &[u8]) {
        self.modulator_tremolo         = (patch[0] & 0b1000_0000) != 0;
        self.modulator_vibrato         = (patch[0] & 0b0100_0000) != 0;
        self.modulator_sustain_enabled = (patch[0] & 0b0010_0000) != 0;
        self.modulator_key_scaling     = (patch[0] & 0b0001_0000) != 0;
        self.modulator_multiplier      = (patch[0] & 0b0000_1111) as usize;

        self.carrier_tremolo         = (patch[1] & 0b1000_0000) != 0;
        self.carrier_vibrato         = (patch[1] & 0b0100_0000) != 0;
        self.carrier_sustain_enabled = (patch[1] & 0b0010_0000) != 0;
        self.carrier_key_scaling     = (patch[1] & 0b0001_0000) != 0;
        self.carrier_multiplier      = (patch[1] & 0b0000_1111) as usize;

        self.modulator_key_level_scaling = ((patch[2] & 0b1100_0000) >> 6) as usize;
        self.modulator_output_level      =  (patch[2] & 0b0011_1111) as u16;

        self.carrier_key_level_scaling = ((patch[3] & 0b1100_0000) >> 6) as usize;
        self.carrier_rectified         =  (patch[3] & 0b0001_0000) != 0;
        self.modulator_rectified       =  (patch[3] & 0b0000_1000) != 0;
        self.feedback                  =   patch[3] & 0b0000_0111;

        self.modulator_attack_rate = (patch[4] & 0b1111_0000) >> 4;
        self.modulator_decay_rate  =  patch[4] & 0b0000_1111;

        self.carrier_attack_rate = (patch[5] & 0b1111_0000) >> 4;
        self.carrier_decay_rate  =  patch[5] & 0b0000_1111;

        self.modulator_sustain_level = (patch[6] & 0b1111_0000) >> 4;
        self.modulator_release_rate  =  patch[6] & 0b0000_1111;

        self.carrier_sustain_level = (patch[7] & 0b1111_0000) >> 4;
        self.carrier_release_rate  =  patch[7] & 0b0000_1111;
    }

    fn effective_rate(&self, given_rate: u8, ks_enabled: bool) -> u8 {
        // effective rates 0..3 and 60..63 are special cases
        if given_rate == 0 {
            return 0;
        }
        if given_rate == 15 {
            return 63;
        }

        let octave_and_fnum_msb = ((self.octave << 1) + (self.fnum >> 8)) as u8;
        let rate_ks = if ks_enabled {
            octave_and_fnum_msb
        } else {
            octave_and_fnum_msb >> 2
        };

        let result = given_rate * 4 + rate_ks;

        // I am rather unsure about this!
        if result > 59 {
            return 59;
        }
        return result;
    }

    fn advance_attack(&self, given_rate: u8, ks_enabled: bool, current_env_level: u8) -> u8  {
        let rate = self.effective_rate(given_rate, ks_enabled);
        match rate / 4 {
            0 | 15 => {}, // Do nothing
            1 ..= 11 => {
                let shift = 13 - (rate / 4);
                let mask = ((1 << shift) - 1) & !3; // ignore lower 2 bits!
                if (self.global_counter & mask) == 0 {
                    let table_index = ((rate & 0x3) * 8) as usize;
                    let row_index = ((self.global_counter >> shift) & 7) as usize;
                    if ADSR_RATE_LUT[table_index + row_index] == 1 {
                        return current_env_level - (current_env_level >> 4) - 1;
                    }
                }
            },
            12 ..= 14 => {
                let table_index = ((rate & 0x3) * 8) as usize;
                let row_index = ((self.global_counter & 0xC) >> 1) as usize;
                let m = 16 - (rate / 4) - ADSR_RATE_LUT[table_index + row_index];
                return current_env_level - (current_env_level >> m) - 1;
            }
            _ => {} // Should be unreachable
        }
        return current_env_level;
    }

    fn shall_we_advance_the_decay_today(&self, given_rate: u8, ks_enabled: bool) -> bool {
        let rate = self.effective_rate(given_rate, ks_enabled);

        let table_index = ((rate & 0x3) * 8) as usize;
        if rate < 56 {
            let shift_amount = 13 - (rate / 4);

            let mask = (1 << shift_amount) - 1;
            if (self.global_counter & mask) == 0 {
                let row_index = ((self.global_counter >> shift_amount) & 0x7) as usize;
                if ADSR_RATE_LUT[table_index + row_index] == 1 {
                    return true;
                }
            }
        } else {
            // weird 16 sample period
            let row_index = ((self.global_counter & 0xC >> 1) | (self.global_counter & 1)) as usize;
            if ADSR_RATE_LUT[table_index + row_index] == 1 {
                return true;
            }
        }

        // Not a chance!
        return false;
    }


    fn update_carrier_adsr(&mut self) {
        let max_env = (self.carrier_env_level >> 2) ==  0x1F;
        if self.carrier_env_state == EnvState::Damp && max_env {
            if self.carrier_attack_rate == 15 {
                self.carrier_env_state = EnvState::Decay;
                self.carrier_env_level = 0;
            } else {
                self.carrier_env_state = EnvState::Attack;
            };
            self.carrier_phase = 0;

            // Also reset the modulator here
            if self.modulator_attack_rate == 15 {
                self.modulator_env_state = EnvState::Decay;
                self.modulator_env_level = 0;
            } else {
                self.modulator_env_state = EnvState::Attack;
                self.modulator_env_level = 127;
            };
            self.modulator_phase = 0;
        } else if self.carrier_env_state == EnvState::Attack && self.carrier_env_level == 0 {
            self.carrier_env_state = EnvState::Decay;
        } else if self.carrier_env_state == EnvState::Decay && ((self.carrier_env_level >> 3) == self.carrier_sustain_level) {
            self.carrier_env_state = EnvState::Sustain;
        }

        let rate = if self.key_on == false {
            // release state
            if self.sustain_mode  {
                5
            } else  if self.carrier_sustain_enabled {
                self.carrier_release_rate
            } else {
                7
            }
        } else {
            match self.carrier_env_state {
                EnvState::Damp => {12},
                EnvState::Attack => {self.carrier_attack_rate },
                EnvState::Decay => {self.carrier_decay_rate },
                EnvState::Sustain => {
                    if self.carrier_sustain_enabled { 0 } else { self.carrier_release_rate }
                }
            }
        };

        if (self.carrier_env_state == EnvState::Attack) && self.key_on == true {
            self.carrier_env_level = self.advance_attack(rate, self.carrier_key_scaling, self.carrier_env_level);
        } else if self.shall_we_advance_the_decay_today(rate, self.carrier_key_scaling) {
            if rate == 0 {
                // Do absolutely nothing
            } else if rate == 15 {
                // Increase the envelope two times (capping at 127)
                self.carrier_env_level += 2;
                if self.carrier_env_level > 127 {
                    self.carrier_env_level = 127;
                }
            } else if self.carrier_env_level < 127 {
                // Increase the envelope just once (usual case)
                self.carrier_env_level += 1;
            }
        }
    }

    fn update_modulator_adsr(&mut self) {
        if self.modulator_env_state == EnvState::Attack && self.modulator_env_level == 0 {
            self.modulator_env_state = EnvState::Decay;
        } else if self.modulator_env_state == EnvState::Decay && ((self.modulator_env_level >> 3) == self.modulator_sustain_level) {
            self.modulator_env_state = EnvState::Sustain;
        }

        let rate = if self.key_on == false {
            0
        } else {
            match self.modulator_env_state {
                EnvState::Damp => {12},
                EnvState::Attack => {self.modulator_attack_rate },
                EnvState::Decay => {self.modulator_decay_rate },
                EnvState::Sustain => {
                    if self.modulator_sustain_enabled { 0 } else { self.modulator_release_rate }
                }
            }
        };

        if (self.modulator_env_state == EnvState::Attack) && self.key_on == true {
            self.modulator_env_level = self.advance_attack(rate, self.modulator_key_scaling, self.modulator_env_level);
        } else if self.shall_we_advance_the_decay_today(rate, self.modulator_key_scaling) {
            if rate == 0 {
                // Do absolutely nothing
            } else if rate == 15 {
                // Increase the envelope two times (capping at 127)
                self.modulator_env_level += 2;
                if self.modulator_env_level > 127 {
                    self.modulator_env_level = 127;
                }
            } else if self.modulator_env_level < 127 {
                // Increase the envelope just once (usual case)
                self.modulator_env_level += 1;
            }
        }
    }

    fn handle_key_on(&mut self, new_key_on: bool) {
        // Transition from 0 -> 1 triggers a new note event
        if self.key_on == false && new_key_on == true {
            // Note: carrier will set modulator state when switching from damp -> attack
            self.carrier_env_state = EnvState::Damp;
        }
        self.key_on = new_key_on;
    }

    pub fn update(&mut self) {
        let carrier_vibrato = if self.carrier_vibrato {
            let upper_fnum = (self.fnum >> 6) as usize;
            FM_LFO_LUT[8 * upper_fnum + self.fm_pos]
        } else {
            0
        };
        let carrier_step_size = (((2 * (self.fnum as i16) + carrier_vibrato) as u32 * MT_LUT[self.carrier_multiplier]) << self.octave) >> 2;
        if self.carrier_phase + carrier_step_size > 0x7FFFF {
            self.last_edge = true;
        }
        self.carrier_phase = (self.carrier_phase + carrier_step_size) & 0x7FFFF;

        let modulator_vibrato = if self.modulator_vibrato {
            let upper_fnum = (self.fnum >> 6) as usize;
            FM_LFO_LUT[8 * upper_fnum + self.fm_pos]
        } else {
            0
        };
        let modulator_step_size = (((2 * (self.fnum as i16) + modulator_vibrato) as u32 * MT_LUT[self.modulator_multiplier]) << self.octave) >> 2;
        self.modulator_phase = (self.modulator_phase + modulator_step_size) & 0x7FFFF;

        if self.am_counter == 0 {
            self.am_pos += 1;
            if self.am_pos >= 210 {
                self.am_pos = 0;
            }
            self.am_counter = 63;
        } else {
            self.am_counter -= 1;
        }

        if self.fm_counter == 0 {
            self.fm_pos += 1;
            if self.fm_pos >= 8 {
                self.fm_pos = 0;
            }
            self.fm_counter = 1023;
        } else {
            self.fm_counter -= 1;
        }

        self.update_carrier_adsr();
        self.update_modulator_adsr();

        self.clock_global_counter();
        self.compute_output();
    }

    pub fn compute_output(&mut self) {
        let feedback = if self.feedback != 0 {
            (self.modulator_previous_0 + self.modulator_previous_1) >> (8 - self.feedback)
        } else {
            0
        };
        let effective_mod_phase = ((self.modulator_phase - 1) & 0x7FFFF) as i32;
        let mod_logsin = self.lookup_logsin((((effective_mod_phase >> 9) + (feedback as i32)) & 0x7FFFF) as usize, self.modulator_rectified);
        let mod_output_attenuation = 32 * self.modulator_output_level;
        let mod_env_attenuation = 16 * self.modulator_env_level as u16;
        let mod_ksl_attenuation = 16 * self.ksl_lut[(self.modulator_key_level_scaling * 8 * 16) + (self.octave as usize * 16) + (self.fnum >> 5) as usize];
        let mod_am_attenuation = if self.modulator_tremolo { 16 * self.am_lut[self.am_pos] } else {0};
        let mod_amount = self.lookup_exp(mod_logsin + mod_output_attenuation + mod_env_attenuation + mod_ksl_attenuation + mod_am_attenuation) >> 1; // drop lowest bit
        self.modulator_previous_0 = self.modulator_previous_1;
        self.modulator_previous_1 = mod_amount;

        let effective_carrier_phase = ((((self.carrier_phase >> 9) as i32) + ((2 * mod_amount as i32)) & 0x7FFFF)) as usize;
        let carrier_logsin = self.lookup_logsin(effective_carrier_phase, self.carrier_rectified);
        let carrier_vol_attenuation = 128 * self.volume;
        let carrier_env_attenuation = 16 * self.carrier_env_level as u16;
        let carrier_ksl_attenuation = 16 * self.ksl_lut[(self.carrier_key_level_scaling * 8 * 16) + (self.octave as usize * 16) + (self.fnum >> 5) as usize];
        let carrier_am_attenuation = if self.carrier_tremolo { 16 * self.am_lut[self.am_pos] } else {0};
        self.current_output = self.lookup_exp(carrier_logsin + carrier_vol_attenuation + carrier_env_attenuation + carrier_ksl_attenuation + carrier_am_attenuation) / 16;
    }

    pub fn output(&self) -> i16 {
        return self.current_output;
    }
}

pub struct Vrc7Audio {
    pub custom_patch: [u8; 8],
    pub channel1: Vrc7AudioChannel,
    pub channel2: Vrc7AudioChannel,
    pub channel3: Vrc7AudioChannel,
    pub channel4: Vrc7AudioChannel,
    pub channel5: Vrc7AudioChannel,
    pub channel6: Vrc7AudioChannel,
    pub current_channel: usize,
    pub delay_counter: u8,
}

impl Vrc7Audio {
    pub fn new() -> Vrc7Audio {
        let thing = Vrc7Audio {
            custom_patch: [0u8; 8],
            channel1: Vrc7AudioChannel::new(1),
            channel2: Vrc7AudioChannel::new(2),
            channel3: Vrc7AudioChannel::new(3),
            channel4: Vrc7AudioChannel::new(4),
            channel5: Vrc7AudioChannel::new(5),
            channel6: Vrc7AudioChannel::new(6),
            current_channel: 1,
            delay_counter: 0,
        };

        //ugly_debug_thing();

        return thing;
    }

    pub fn clock(&mut self) {
        if self.delay_counter == 0 {
            match self.current_channel {
                0 => self.channel1.update(),
                1 => self.channel2.update(),
                2 => self.channel3.update(),
                3 => self.channel4.update(),
                4 => self.channel5.update(),
                5 => self.channel6.update(),
                _ => {} // unreachable
            }
            self.current_channel += 1;
            if self.current_channel >= 6 {
                self.current_channel = 0;
            }
            self.delay_counter = 5;
        } else {
            self.delay_counter -= 1;
        }
    }

    pub fn output(&self) -> i16 {
        let mut combined_output = 0;
        if !self.channel1.debug_disable {
            combined_output += self.channel1.output();
        }
        if !self.channel2.debug_disable {
            combined_output += self.channel2.output();
        }
        if !self.channel3.debug_disable {
            combined_output += self.channel3.output();
        }
        if !self.channel4.debug_disable {
            combined_output += self.channel4.output();
        }
        if !self.channel5.debug_disable {
            combined_output += self.channel5.output();
        }
        if !self.channel6.debug_disable {
            combined_output += self.channel6.output();
        }
        return combined_output;
    }

    pub fn refresh_custom_patch(&mut self) {
        if self.channel1.instrument_index == 0 {
            self.channel1.load_patch(&self.custom_patch);
        }
        if self.channel2.instrument_index == 0 {
            self.channel2.load_patch(&self.custom_patch);
        }
        if self.channel3.instrument_index == 0 {
            self.channel3.load_patch(&self.custom_patch);
        }
        if self.channel4.instrument_index == 0 {
            self.channel4.load_patch(&self.custom_patch);
        }
        if self.channel5.instrument_index == 0 {
            self.channel5.load_patch(&self.custom_patch);
        }
        if self.channel6.instrument_index == 0 {
            self.channel6.load_patch(&self.custom_patch);
        }
    }

    pub fn write(&mut self, address: u8, data: u8) {
        match address {
            0x00 => {self.custom_patch[0] = data; self.refresh_custom_patch()},
            0x01 => {self.custom_patch[1] = data; self.refresh_custom_patch()},
            0x02 => {self.custom_patch[2] = data; self.refresh_custom_patch()},
            0x03 => {self.custom_patch[3] = data; self.refresh_custom_patch()},
            0x04 => {self.custom_patch[4] = data; self.refresh_custom_patch()},
            0x05 => {self.custom_patch[5] = data; self.refresh_custom_patch()},
            0x06 => {self.custom_patch[6] = data; self.refresh_custom_patch()},
            0x07 => {self.custom_patch[7] = data; self.refresh_custom_patch()},
            0x10 => {self.channel1.fnum = (self.channel1.fnum & 0xFF00) + (data as u32)},
            0x11 => {self.channel2.fnum = (self.channel2.fnum & 0xFF00) + (data as u32)},
            0x12 => {self.channel3.fnum = (self.channel3.fnum & 0xFF00) + (data as u32)},
            0x13 => {self.channel4.fnum = (self.channel4.fnum & 0xFF00) + (data as u32)},
            0x14 => {self.channel5.fnum = (self.channel5.fnum & 0xFF00) + (data as u32)},
            0x15 => {self.channel6.fnum = (self.channel6.fnum & 0xFF00) + (data as u32)},
            0x20 => {
                self.channel1.fnum = (self.channel1.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel1.octave = ((data & 0b1110) >> 1) as u32;
                self.channel1.handle_key_on((data & 0b1_0000) != 0);
                self.channel1.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x21 => {
                self.channel2.fnum = (self.channel2.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel2.octave = ((data & 0b1110) >> 1) as u32;
                self.channel2.handle_key_on((data & 0b1_0000) != 0);
                self.channel2.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x22 => {
                self.channel3.fnum = (self.channel3.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel3.octave = ((data & 0b1110) >> 1) as u32;
                self.channel3.handle_key_on((data & 0b1_0000) != 0);
                self.channel3.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x23 => {
                self.channel4.fnum = (self.channel4.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel4.octave = ((data & 0b1110) >> 1) as u32;
                self.channel4.handle_key_on((data & 0b1_0000) != 0);
                self.channel4.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x24 => {
                self.channel5.fnum = (self.channel5.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel5.octave = ((data & 0b1110) >> 1) as u32;
                self.channel5.handle_key_on((data & 0b1_0000) != 0);
                self.channel5.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x25 => {
                self.channel6.fnum = (self.channel6.fnum & 0x00FF) + (((data & 0b1) as u32) << 8);
                self.channel6.octave = ((data & 0b1110) >> 1) as u32;
                self.channel6.handle_key_on((data & 0b1_0000) != 0);
                self.channel6.sustain_mode = (data & 0b10_0000) != 0;
            },
            0x30 => {
                self.channel1.volume = (data & 0xF) as u16;
                self.channel1.instrument_index = (data & 0xF0) >> 4;
                if self.channel1.instrument_index == 0 {
                    self.channel1.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel1.instrument_index - 1) * 8) as usize;
                    self.channel1.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            0x31 => {
                self.channel2.volume = (data & 0xF) as u16;
                self.channel2.instrument_index = (data & 0xF0) >> 4;
                if self.channel2.instrument_index == 0 {
                    self.channel2.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel2.instrument_index - 1) * 8) as usize;
                    self.channel2.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            0x32 => {
                self.channel3.volume = (data & 0xF) as u16;
                self.channel3.instrument_index = (data & 0xF0) >> 4;
                if self.channel3.instrument_index == 0 {
                    self.channel3.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel3.instrument_index - 1) * 8) as usize;
                    self.channel3.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            0x33 => {
                self.channel4.volume = (data & 0xF) as u16;
                self.channel4.instrument_index = (data & 0xF0) >> 4;
                if self.channel4.instrument_index == 0 {
                    self.channel4.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel4.instrument_index - 1) * 8) as usize;
                    self.channel4.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            0x34 => {
                self.channel5.volume = (data & 0xF) as u16;
                self.channel5.instrument_index = (data & 0xF0) >> 4;
                if self.channel5.instrument_index == 0 {
                    self.channel5.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel5.instrument_index - 1) * 8) as usize;
                    self.channel5.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            0x35 => {
                self.channel6.volume = (data & 0xF) as u16;
                self.channel6.instrument_index = (data & 0xF0) >> 4;
                if self.channel6.instrument_index == 0 {
                    self.channel6.load_patch(&self.custom_patch);
                } else {
                    let patch_index = ((self.channel6.instrument_index - 1) * 8) as usize;
                    self.channel6.load_patch(&DEFAULT_PATCH_TABLE[patch_index .. patch_index + 8]);
                }
            },
            _ => {}
        }
    }

    pub fn record_output(&mut self) {
        self.channel1.record_current_output();
        self.channel2.record_current_output();
        self.channel3.record_current_output();
        self.channel4.record_current_output();
        self.channel5.record_current_output();
        self.channel6.record_current_output();
    }
}

impl AudioChannelState for Vrc7AudioChannel {
    fn name(&self) -> String {
        return format!("FM {}", self.channel_index);
    }

    fn chip(&self) -> String {
        return "VRC7".to_string();
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
        return -2048;
    }

    fn max_sample(&self) -> i16 {
        return 2048;
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
            self.fnum > 0 &&
                self.carrier_env_level < 120 &&
                self.volume < 0xF;
    }

    fn rate(&self) -> PlaybackRate {
        let effective_frequency = (49716.0 * self.fnum as f32) / (2.0_f32.powf(19.0 - self.octave as f32));
        return PlaybackRate::FundamentalFrequency {frequency: effective_frequency};
    }

    fn volume(&self) -> Option<Volume> {
        let approximate_volume = self.lookup_exp(self.lookup_logsin(0, false) + 128 * self.volume + 16 * self.carrier_env_level as u16) + 1; // If we're playing at all, display *something*
        return Some(Volume::VolumeIndex{ index: approximate_volume as usize, max: 12 }); //  max chosen arbitrary to get a decent-ish relative scale
    }

    fn timbre(&self) -> Option<Timbre> {
        return Some(Timbre::PatchIndex{ index: self.instrument_index as usize, max: 15 });
    }
}

