// Vrc6, 
// Reference capabilities: https://wiki.nesdev.com/w/index.php/VRC6

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

pub struct Vrc6PulseChannel {
    pub name: String,
    pub debug_disable: bool,
    pub enabled: bool,
    pub duty_compare: u8,
    pub duty_counter: u8,
    pub volume: u8,
    pub period_initial: u16,
    pub period_current: u16,
    pub halt: bool,
    pub scale_256: bool,
    pub scale_16: bool,
    pub output_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub last_edge: bool,
    pub debug_filter: filters::HighPassIIR,
}

impl Vrc6PulseChannel {
    pub fn new(channel_name: &str) -> Vrc6PulseChannel {
        return Vrc6PulseChannel {
            name: String::from(channel_name),
            debug_disable: false,
            enabled: false,
            duty_compare: 16,
            duty_counter: 0,
            volume: 0,
            period_initial: 0,
            period_current: 0,
            halt: true,
            scale_256: false,
            scale_16: false,
            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0),
        };
    }

    pub fn _clock_duty_generator(&mut self) {
        if self.duty_counter == 0 {
            self.duty_counter = 15;
            self.last_edge = true;
        } else {
            self.duty_counter -= 1;
        }
    }

    pub fn _reload_period_counter(&mut self) {
        if self.scale_256 {
            self.period_current = self.period_initial >> 8;
        } else if self.scale_16 {
            self.period_current = self.period_initial >> 4;
        } else {
            self.period_current = self.period_initial;
        }
    }

    pub fn clock(&mut self) {
        if self.halt || !self.enabled {
            return;
        }
        if self.period_current == 0 {
            self._clock_duty_generator();
            self._reload_period_counter();
        } else {
            self.period_current -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if self.enabled && self.duty_compare >= self.duty_counter {
            return self.volume;
        }
        return 0;
    }

    pub fn write_register(&mut self, index: u8, data: u8) {
        match index {
            0 => {
                let direct_volume_mode = (data & 0b1000_0000) != 0;
                if direct_volume_mode {
                    self.duty_compare = 16;
                } else {
                    self.duty_compare = (data & 0b0111_0000) >> 4;
                }
                self.volume = data & 0b0000_1111;
            },
            1 => {
                self.period_initial = (self.period_initial & 0xFF00) + (data as u16);
            },
            2 => {
                self.enabled = (data & 0b1000_0000) != 0;
                self.period_initial = (self.period_initial & 0x00FF) + (((data & 0x0F) as u16) << 8);
                if !self.enabled {
                    // reset phase entirely
                    self.duty_counter = 15;
                    self.period_current = self.period_initial;
                }
            },
            3 => {
                self.halt      = (data & 0b0000_0001) != 0;
                self.scale_16  = (data & 0b0000_0010) != 0;
                self.scale_256 = (data & 0b0000_0100) != 0;
            },
            _ => {}
        }
    }
}

impl AudioChannelState for Vrc6PulseChannel {
    fn name(&self) -> String {
        return self.name.clone();
    }

    fn chip(&self) -> String {
        return "VRC6".to_string();
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
            (self.enabled) &&
            (!self.halt) &&
            (self.duty_compare != 16) &&
            (self.volume > 0);
    }

    fn rate(&self) -> PlaybackRate {
        let frequency = 1_789_773.0 / (16.0 * (self.period_initial as f32 + 1.0));
        return PlaybackRate::FundamentalFrequency {frequency: frequency};
    }

    fn volume(&self) -> Option<Volume> {
        return Some(Volume::VolumeIndex{ index: self.volume as usize, max: 15 });
    }

    fn timbre(&self) -> Option<Timbre> {
        return Some(Timbre::DutyIndex{ index: self.duty_compare as usize, max: 7 });
    }
}

pub struct Vrc6SawtoothChannel {
    pub enabled: bool,
    pub debug_disable: bool,
    pub accumulator_rate: u8,
    pub accumulator_step: u8,
    pub accumulator: u8,
    pub period_initial: u16,
    pub period_current: u16,
    pub halt: bool,
    pub scale_256: bool,
    pub scale_16: bool,
    pub output_buffer: RingBuffer,
    pub edge_buffer: RingBuffer,
    pub last_edge: bool,
    pub debug_filter: filters::HighPassIIR,
}

impl Vrc6SawtoothChannel {
    pub fn new() -> Vrc6SawtoothChannel {
        return Vrc6SawtoothChannel {
            enabled: false,
            debug_disable: false,
            accumulator_rate: 0,
            accumulator_step: 0,
            accumulator: 0,
            period_initial: 0,
            period_current: 0,
            halt: true,
            scale_256: false,
            scale_16: false,
            output_buffer: RingBuffer::new(32768),
            edge_buffer: RingBuffer::new(32768),
            last_edge: false,
            debug_filter: filters::HighPassIIR::new(44100.0, 300.0),
        };
    }

    pub fn _clock_accumulator(&mut self) {
        self.accumulator_step += 1;
        if self.accumulator_step >= 14 {
            self.accumulator_step = 0;
            self.accumulator = 0;
            self.last_edge = true;
        } else {
            // Only take action on EVEN steps:
            if (self.accumulator_step & 0b1) == 0 {
                self.accumulator = self.accumulator.wrapping_add(self.accumulator_rate);
            }
        }
    }

    pub fn _reload_period_counter(&mut self) {
        if self.scale_256 {
            self.period_current = self.period_initial >> 8;
        } else if self.scale_16 {
            self.period_current = self.period_initial >> 4;
        } else {
            self.period_current = self.period_initial;
        }
    }

    pub fn clock(&mut self) {
        if self.halt || !self.enabled {
            return;
        }
        if self.period_current == 0 {
            self._clock_accumulator();
            self._reload_period_counter();
        } else {
            self.period_current -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        return self.accumulator >> 3;
    }

    pub fn write_register(&mut self, index: u8, data: u8) {
        match index {
            0 => {
                self.accumulator_rate = data & 0b0011_1111;
            },
            1 => {
                self.period_initial = (self.period_initial & 0xFF00) + (data as u16);
            },
            2 => {
                self.enabled = (data & 0b1000_0000) != 0;
                self.period_initial = (self.period_initial & 0x00FF) + (((data & 0x0F) as u16) << 8);
                if !self.enabled {
                    // reset phase entirely
                    self.accumulator = 0;
                    self.accumulator_step = 0;
                }
            },
            3 => {
                self.halt      = (data & 0b0000_0001) != 0;
                self.scale_16  = (data & 0b0000_0010) != 0;
                self.scale_256 = (data & 0b0000_0100) != 0;
            },
            _ => {}
        }
    }
}

impl AudioChannelState for Vrc6SawtoothChannel {
    fn name(&self) -> String {
        return "Sawtooth".to_string();
    }

    fn chip(&self) -> String {
        return "VRC6".to_string();
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
        return -124;
    }

    fn max_sample(&self) -> i16 {
        return 124;
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
            (self.enabled) &&
            (!self.halt) &&
            (self.accumulator_rate > 0);
    }

    fn rate(&self) -> PlaybackRate {
        let frequency = 1_789_773.0 / (14.0 * (self.period_initial as f32 + 1.0));
        return PlaybackRate::FundamentalFrequency {frequency: frequency};
    }

    fn volume(&self) -> Option<Volume> {
        if self.accumulator_rate <= 42 {
            // Normal volume, report this directly
            return Some(Volume::VolumeIndex{ index: self.accumulator_rate as usize, max: 42 });
        } else {
            // distorted volume, report this near the top end; these are always
            // quite loud
            let distorted_volume = self.accumulator_rate - 21;
            return Some(Volume::VolumeIndex{ index: distorted_volume as usize, max: 42 });
        }
    }

    fn timbre(&self) -> Option<Timbre> {
        if self.accumulator_rate <= 42 {
            return Some(Timbre::DutyIndex{ index: 0 as usize, max: 1 });
        } else {
            return Some(Timbre::DutyIndex{ index: 1 as usize, max: 1 });
        }
        
    }
}

pub struct Vrc6 {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,
    pub vram: Vec<u8>,
    pub prg_ram_enable: bool,
    pub prg_bank_16: usize,
    pub prg_bank_8: usize,
    pub r: Vec<usize>,
    pub ppu_banking_mode: u8,
    pub mirroring_mode: u8,
    pub nametable_chrrom: bool,
    pub chr_a10_rules: bool,
    pub mirroring: Mirroring,
    pub mapper_number: u16,
    pub b003_shadow: u8,

    pub irq_scanline_prescaler: i16,
    pub irq_latch: u8,
    pub irq_scanline_mode: bool,
    pub irq_enable: bool,
    pub irq_enable_after_acknowledgement: bool,
    pub irq_pending: bool,
    pub irq_counter: u8,

    pub pulse1: Vrc6PulseChannel,
    pub pulse2: Vrc6PulseChannel,
    pub sawtooth: Vrc6SawtoothChannel,
}

impl Vrc6 {
    pub fn from_ines(ines: INesCartridge) -> Result<Vrc6, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Vrc6 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            vram: vec![0u8; 0x1000],
            prg_ram_enable: false,
            prg_bank_16: 0,
            prg_bank_8: 0,
            r: vec![0usize; 8],
            ppu_banking_mode: 0,
            mirroring_mode: 0,
            nametable_chrrom: false,
            chr_a10_rules: false,
            mirroring: ines.header.mirroring(),
            mapper_number: ines.header.mapper_number(),
            b003_shadow: 0,

            irq_scanline_prescaler: 0,
            irq_latch: 0,
            irq_scanline_mode: false,
            irq_enable: false,
            irq_enable_after_acknowledgement: false,
            irq_pending: false,
            irq_counter: 0,

            pulse1: Vrc6PulseChannel::new("Pulse 1"),
            pulse2: Vrc6PulseChannel::new("Pulse 2"),
            sawtooth: Vrc6SawtoothChannel::new(),
        });
    }

    fn _chr_mode_0(&self, address: u16) -> Option<u8> {
        // All 1k banks
        match address {
            0x0000 ..= 0x03FF => self.chr.banked_read(0x400, self.r[0], address as usize -  0x0000),
            0x0400 ..= 0x07FF => self.chr.banked_read(0x400, self.r[1], address as usize -  0x0400),
            0x0800 ..= 0x0BFF => self.chr.banked_read(0x400, self.r[2], address as usize -  0x0800),
            0x0C00 ..= 0x0FFF => self.chr.banked_read(0x400, self.r[3], address as usize -  0x0C00),
            0x1000 ..= 0x13FF => self.chr.banked_read(0x400, self.r[4], address as usize -  0x1000),
            0x1400 ..= 0x17FF => self.chr.banked_read(0x400, self.r[5], address as usize -  0x1400),
            0x1800 ..= 0x1BFF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x1800),
            0x1C00 ..= 0x1FFF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x1C00),
            _ => None // never reached
        }
    }

    fn _chr_mode_1(&self, address: u16) -> Option<u8> {
        // All 2k banks, with differing A10 behavior        
        if self.chr_a10_rules {
            //2k banks use PPU A10, ignore low bit of register
            match address {
                0x0000 ..= 0x07FF => self.chr.banked_read(0x800, (self.r[0] & 0xFE) >> 1, address as usize -  0x0000),
                0x0800 ..= 0x0FFF => self.chr.banked_read(0x800, (self.r[1] & 0xFE) >> 1, address as usize -  0x0800),
                0x1000 ..= 0x17FF => self.chr.banked_read(0x800, (self.r[2] & 0xFE) >> 1, address as usize -  0x1000),
                0x1800 ..= 0x1FFF => self.chr.banked_read(0x800, (self.r[3] & 0xFE) >> 1, address as usize -  0x1800),

                _ => None // never reached
            }
        } else {
            // Low bit of register determines A10, effectively duplicating 1k banks, similar to 1k mode
            match address {
                0x0000 ..= 0x03FF => self.chr.banked_read(0x400, self.r[0], address as usize -  0x0000),
                0x0400 ..= 0x07FF => self.chr.banked_read(0x400, self.r[0], address as usize -  0x0400),
                0x0800 ..= 0x0BFF => self.chr.banked_read(0x400, self.r[1], address as usize -  0x0800),
                0x0C00 ..= 0x0FFF => self.chr.banked_read(0x400, self.r[1], address as usize -  0x0C00),
                0x1000 ..= 0x13FF => self.chr.banked_read(0x400, self.r[2], address as usize -  0x1000),
                0x1400 ..= 0x17FF => self.chr.banked_read(0x400, self.r[2], address as usize -  0x1400),
                0x1800 ..= 0x1BFF => self.chr.banked_read(0x400, self.r[3], address as usize -  0x1800),
                0x1C00 ..= 0x1FFF => self.chr.banked_read(0x400, self.r[3], address as usize -  0x1C00),
                _ => None // never reached
            }
        }
    }

    fn _chr_mode_23(&self, address: u16) -> Option<u8> {
        // Essentially a mix, mode 0 for the upper half, with 2x 2k banks in the lower half that behave similarly to mode 1
        // but pull from R4-R5 instead
        match address {
            0x0000 ..= 0x0FFF => self._chr_mode_0(address),
            0x1000 ..= 0x1FFF => {
                if self.chr_a10_rules {
                    //2k banks use PPU A10, ignore low bit of register
                    match address {
                        0x1000 ..= 0x17FF => self.chr.banked_read(0x800, (self.r[4] & 0xFE) >> 1, address as usize -  0x1000),
                        0x1800 ..= 0x1FFF => self.chr.banked_read(0x800, (self.r[5] & 0xFE) >> 1, address as usize -  0x1800),
                        _ => None // never reached
                    }
                } else {
                    // Low bit of register determines A10, effectively duplicating 1k banks, similar to 1k mode
                    match address {
                        0x1000 ..= 0x13FF => self.chr.banked_read(0x400, self.r[4], address as usize -  0x1000),
                        0x1400 ..= 0x17FF => self.chr.banked_read(0x400, self.r[4], address as usize -  0x1400),
                        0x1800 ..= 0x1BFF => self.chr.banked_read(0x400, self.r[5], address as usize -  0x1800),
                        0x1C00 ..= 0x1FFF => self.chr.banked_read(0x400, self.r[5], address as usize -  0x1C00),
                        _ => None // never reached
                    }
                }
            }
            _ => None // never reached
        }
    }

    fn _mirroring_mode_0_read(&self, address: u16) -> Option<u8> {
        let mirrored_address = address & 0x2FFF;
        if self.nametable_chrrom {
            match self.mirroring_mode {
                0 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                1 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                2 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                3 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                _ => None
            }
        } else {
            match self.mirroring_mode {
                0 => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                1 => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                2 => Some(self.vram[mirroring::one_screen_lower(address) as usize]),
                3 => Some(self.vram[mirroring::one_screen_upper(address) as usize]),
                _ => None
            }
        }
    }

    fn _mirroring_mode_0_write(&mut self, address: u16, data: u8) {
        if self.nametable_chrrom {
            println!("Attempt to write to CHR ROM nametables!");
        } else {
            match self.mirroring_mode {
                0 => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                1 => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                2 => self.vram[mirroring::one_screen_lower(address) as usize] = data,
                3 => self.vram[mirroring::one_screen_upper(address) as usize] = data,
                _ => {}
            }
        }
    }

    fn _mirroring_mode_1_read(&self, address: u16) -> Option<u8> {
        let mirrored_address = address & 0x2FFF;
        let masked_address = (mirrored_address & 0b0011_1111_1111) as usize;
        if self.nametable_chrrom {
            match mirrored_address {
                0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[4], address as usize -  0x2000),
                0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[5], address as usize -  0x2400), 
                0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x2800), 
                0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x2C00),
                _ => None // never reached
            }
        } else {
            let r4_lsb = (self.r[4] & 0x1) as usize;
            let r5_lsb = (self.r[5] & 0x1) as usize;
            let r6_lsb = (self.r[6] & 0x1) as usize;
            let r7_lsb = (self.r[7] & 0x1) as usize;
            match mirrored_address {
                0x2000 ..= 0x23FF => Some(self.vram[masked_address + (r4_lsb << 10)]),
                0x2400 ..= 0x27FF => Some(self.vram[masked_address + (r5_lsb << 10)]),
                0x2800 ..= 0x2BFF => Some(self.vram[masked_address + (r6_lsb << 10)]),
                0x2C00 ..= 0x2FFF => Some(self.vram[masked_address + (r7_lsb << 10)]),
                _ => None // never reached
            }
        }
    }

    fn _mirroring_mode_1_write(&mut self, address: u16, data: u8) {
        if self.nametable_chrrom {
            println!("Attempt to write to CHR ROM nametables!");
        } else {
            let mirrored_address = address & 0x2FFF;
            let masked_address = (mirrored_address & 0b0011_1111_1111) as usize;

            let r4_lsb = (self.r[4] & 0x1) as usize;
            let r5_lsb = (self.r[5] & 0x1) as usize;
            let r6_lsb = (self.r[6] & 0x1) as usize;
            let r7_lsb = (self.r[7] & 0x1) as usize;

            match mirrored_address {
                0x2000 ..= 0x23FF => self.vram[masked_address + (r4_lsb << 10)] = data,
                0x2400 ..= 0x27FF => self.vram[masked_address + (r5_lsb << 10)] = data,
                0x2800 ..= 0x2BFF => self.vram[masked_address + (r6_lsb << 10)] = data,
                0x2C00 ..= 0x2FFF => self.vram[masked_address + (r7_lsb << 10)] = data,
                _ => {} // never reached
            }
        }
    }

    fn _mirroring_mode_2_read(&self, address: u16) -> Option<u8> {
        let mirrored_address = address & 0x2FFF;
        let masked_address = (mirrored_address & 0b0011_1111_1111) as usize;
        if self.nametable_chrrom {
            match self.mirroring_mode {
                0 | 2=> {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                1 | 3=> {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[6], address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7], address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                _ => None
            }
        } else {
            let r6_lsb = (self.r[6] & 0x1) as usize;
            let r7_lsb = (self.r[7] & 0x1) as usize;

            match self.mirroring_mode {
                0 | 2 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => Some(self.vram[masked_address + (r6_lsb << 10)]),
                        0x2400 ..= 0x27FF => Some(self.vram[masked_address + (r7_lsb << 10)]),
                        0x2800 ..= 0x2BFF => Some(self.vram[masked_address + (r6_lsb << 10)]),
                        0x2C00 ..= 0x2FFF => Some(self.vram[masked_address + (r7_lsb << 10)]),
                        _ => None // never reached
                    }
                },
                1 | 3 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => Some(self.vram[masked_address + (r6_lsb << 10)]),
                        0x2400 ..= 0x27FF => Some(self.vram[masked_address + (r6_lsb << 10)]),
                        0x2800 ..= 0x2BFF => Some(self.vram[masked_address + (r7_lsb << 10)]),
                        0x2C00 ..= 0x2FFF => Some(self.vram[masked_address + (r7_lsb << 10)]),
                        _ => None // never reached
                    }
                },
                _ => None
            }
        }
    }

    fn _mirroring_mode_2_write(&mut self, address: u16, data: u8) {
        let mirrored_address = address & 0x2FFF;
        let masked_address = (mirrored_address & 0b0011_1111_1111) as usize;
        if self.nametable_chrrom {
            match self.mirroring_mode {
                0 | 2=> {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_write(0x400, self.r[6], address as usize -  0x2000, data),
                        0x2400 ..= 0x27FF => self.chr.banked_write(0x400, self.r[7], address as usize -  0x2400, data), 
                        0x2800 ..= 0x2BFF => self.chr.banked_write(0x400, self.r[6], address as usize -  0x2800, data), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_write(0x400, self.r[7], address as usize -  0x2C00, data),
                        _ => {} // never reached
                    }
                },
                1 | 3=> {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_write(0x400, self.r[6], address as usize -  0x2000, data),
                        0x2400 ..= 0x27FF => self.chr.banked_write(0x400, self.r[6], address as usize -  0x2400, data), 
                        0x2800 ..= 0x2BFF => self.chr.banked_write(0x400, self.r[7], address as usize -  0x2800, data), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_write(0x400, self.r[7], address as usize -  0x2C00, data),
                        _ => {} // never reached
                    }
                },
                _ => {}
            }
        } else {
            let r6_lsb = (self.r[6] & 0x1) as usize;
            let r7_lsb = (self.r[7] & 0x1) as usize;

            match self.mirroring_mode {
                0 | 2 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.vram[masked_address + (r6_lsb << 10)] = data,
                        0x2400 ..= 0x27FF => self.vram[masked_address + (r7_lsb << 10)] = data,
                        0x2800 ..= 0x2BFF => self.vram[masked_address + (r6_lsb << 10)] = data,
                        0x2C00 ..= 0x2FFF => self.vram[masked_address + (r7_lsb << 10)] = data,
                        _ => {} // never reached
                    }
                },
                1 | 3 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.vram[masked_address + (r6_lsb << 10)] = data,
                        0x2400 ..= 0x27FF => self.vram[masked_address + (r6_lsb << 10)] = data,
                        0x2800 ..= 0x2BFF => self.vram[masked_address + (r7_lsb << 10)] = data,
                        0x2C00 ..= 0x2FFF => self.vram[masked_address + (r7_lsb << 10)] = data,
                        _ => {} // never reached
                    }
                },
                _ => {}
            }
        }
    }

    fn _mirroring_mode_3_read(&self, address: u16) -> Option<u8> {
        //println!("mode 3 read with address: {}", address);
        let mirrored_address = address & 0x2FFF;
        if self.nametable_chrrom {
            match self.mirroring_mode {
                0 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                1 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                2 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[6] | 0x01, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] | 0x01, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                3 => {
                    match mirrored_address {
                        0x2000 ..= 0x23FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2000),
                        0x2400 ..= 0x27FF => self.chr.banked_read(0x400, self.r[6] & 0xFE, address as usize -  0x2400), 
                        0x2800 ..= 0x2BFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2800), 
                        0x2C00 ..= 0x2FFF => self.chr.banked_read(0x400, self.r[7] & 0xFE, address as usize -  0x2C00),
                        _ => None // never reached
                    }
                },
                _ => None
            }
        } else {
            match self.mirroring_mode {
                0 => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                1 => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                2 => Some(self.vram[mirroring::one_screen_upper(address) as usize]),
                3 => Some(self.vram[mirroring::one_screen_lower(address) as usize]),
                _ => None
            }
        }
    }

    fn _mirroring_mode_3_write(&mut self, address: u16, data: u8) {
        if self.nametable_chrrom {
            println!("Attempt to write to CHR ROM nametables!");
        } else {
            match self.mirroring_mode {
                0 => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                1 => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                2 => self.vram[mirroring::one_screen_upper(address) as usize] = data,
                3 => self.vram[mirroring::one_screen_lower(address) as usize] = data,
                _ => {}
            }
        }
    }

     fn _a10_chr_address(&self, address: u16) -> usize {
        let mirrored_address = address & 0x2FFF;
        let masked_address = (mirrored_address & 0b0011_1111_1111) as usize;

        match self.b003_shadow & 0xF {
            0x0 | 0x6 | 0x7 | 0x8 | 0xE | 0xF => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => (self.r[6] << 10) + masked_address,
                    0x2400 ..= 0x27FF => (self.r[6] << 10) + masked_address,
                    0x2800 ..= 0x2BFF => (self.r[7] << 10) + masked_address,
                    0x2C00 ..= 0x2FFF => (self.r[7] << 10) + masked_address,
                    _ => 0 // unreachable
                }
            },
            0x1 | 0x5 | 0x9 | 0xD => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => (self.r[4] << 10) + masked_address,
                    0x2400 ..= 0x27FF => (self.r[5] << 10) + masked_address,
                    0x2800 ..= 0x2BFF => (self.r[6] << 10) + masked_address,
                    0x2C00 ..= 0x2FFF => (self.r[7] << 10) + masked_address,
                    _ => 0 // unreachable
                }
            }
            0x2 | 0x3 | 0x4 | 0xA | 0xB | 0xC => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => (self.r[6] << 10) + masked_address,
                    0x2400 ..= 0x27FF => (self.r[7] << 10) + masked_address,
                    0x2800 ..= 0x2BFF => (self.r[6] << 10) + masked_address,
                    0x2C00 ..= 0x2FFF => (self.r[7] << 10) + masked_address,
                    _ => 0 // unreachable
                }   
            }
            _ => 0 // unreachable
        }
    }   

    fn _a10_nametable_address(&self, address: u16) -> usize {
        let r4_lsb = (self.r[4] & 0x1) as usize;
        let r5_lsb = (self.r[5] & 0x1) as usize;
        let r6_lsb = (self.r[6] & 0x1) as usize;
        let r7_lsb = (self.r[7] & 0x1) as usize;
        let mirrored_address = address & 0x2FFF;
        let masked_address = (address & 0b0011_1111_1111) as usize;

        match self.b003_shadow & 0xF {
            0x0 | 0x6 | 0x7 | 0x8 | 0xE | 0xF => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => masked_address + (r6_lsb << 10),
                    0x2400 ..= 0x27FF => masked_address + (r6_lsb << 10),
                    0x2800 ..= 0x2BFF => masked_address + (r7_lsb << 10),
                    0x2C00 ..= 0x2FFF => masked_address + (r7_lsb << 10),
                    _ => 0 // unreachable
                }
            },
            0x1 | 0x5 | 0x9 | 0xD => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => masked_address + (r4_lsb << 10),
                    0x2400 ..= 0x27FF => masked_address + (r5_lsb << 10),
                    0x2800 ..= 0x2BFF => masked_address + (r6_lsb << 10),
                    0x2C00 ..= 0x2FFF => masked_address + (r7_lsb << 10),
                    _ => 0 // unreachable
                }
            }
            0x2 | 0x3 | 0x4 | 0xA | 0xB | 0xC => {
                match mirrored_address {
                    0x2000 ..= 0x23FF => masked_address + (r6_lsb << 10),
                    0x2400 ..= 0x27FF => masked_address + (r7_lsb << 10),
                    0x2800 ..= 0x2BFF => masked_address + (r6_lsb << 10),
                    0x2C00 ..= 0x2FFF => masked_address + (r7_lsb << 10),
                    _ => 0 // unreachable
                }   
            }
            _ => 0 // unreachable
        }
    }

    fn _a10_nametable_read(&self, address: u16) -> Option<u8> {
        if self.nametable_chrrom {
            let a10_rules_address = self._a10_chr_address(address);
            return self.chr.wrapping_read(a10_rules_address);
        } else {
            let a10_rules_address = self._a10_nametable_address(address);
            return Some(self.vram[a10_rules_address]);
        }
    }

    fn _a10_nametable_write(&mut self, address: u16, data: u8) {
        if self.nametable_chrrom {
            println!("Attempt to write to CHR ROM nametables!");
            return;
        }
        let a10_rules_address = self._a10_nametable_address(address);
        self.vram[a10_rules_address] = data;
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

impl Mapper for Vrc6 {
    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn clock_cpu(&mut self) {
        if self.irq_enable {
            if self.irq_scanline_mode {
                self._clock_irq_prescaler();
            } else {
                self._clock_irq_counter();
            }
        }
        self.pulse1.clock();
        self.pulse2.clock();
        self.sawtooth.clock();
    }

    fn mix_expansion_audio(&self, nes_sample: f32) -> f32 {
        let pulse_1_output = if !self.pulse1.debug_disable {self.pulse1.output() as f32} else {0.0};
        let pulse_2_output = if !self.pulse2.debug_disable {self.pulse2.output() as f32} else {0.0};
        let sawtooth_output = if !self.sawtooth.debug_disable {self.sawtooth.output() as f32} else {0.0};
        let vrc6_combined_sample = (pulse_1_output + pulse_2_output + sawtooth_output) / 61.0;

        let nes_pulse_full_volume = 95.88 / ((8128.0 / 15.0) + 100.0);
        let vrc6_pulse_full_volume = 15.0 / 61.0;
        let vrc6_weight = nes_pulse_full_volume / vrc6_pulse_full_volume;

        return 
            (vrc6_combined_sample * vrc6_weight) + 
            nes_sample;
    }

    fn irq_flag(&self) -> bool {
        return self.irq_pending;
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x6000 ..= 0x7FFF => self.prg_ram.wrapping_read(address as usize - 0x6000),
            0x8000 ..= 0xBFFF => self.prg_rom.banked_read(0x4000, self.prg_bank_16, address as usize -  0x8000),
            0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, self.prg_bank_8, address as usize -  0xC000),
            0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF, address as usize -  0xE000),
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x6000 ..= 0x7FFF => {
                if self.prg_ram_enable {
                    self.prg_ram.wrapping_write(address as usize - 0x6000, data)
                }
            },
            _ => {}
        }
        let mut masked_address = address & 0b1111_0000_0000_0011;
        if self.mapper_number == 26 {
            // switch the roles of a1 and a0
            let a1 = (masked_address & 0b10) >> 1;
            let a0 = masked_address & 0b01;
            masked_address = (masked_address & 0b1111_0000_0000_0000) + (a0 << 1) + a1;
        }
        match masked_address {
            0x8000 ..= 0x8003 => {
                self.prg_bank_16 = data as usize & 0x0F;
            },
            0x9000 => {self.pulse1.write_register(0, data);},
            0x9001 => {self.pulse1.write_register(1, data);},
            0x9002 => {self.pulse1.write_register(2, data);},
            0x9003 => {
                self.pulse1.write_register(3, data);
                self.pulse2.write_register(3, data);
                self.sawtooth.write_register(3, data);
            },
            0xA000 => {self.pulse2.write_register(0, data);},
            0xA001 => {self.pulse2.write_register(1, data);},
            0xA002 => {self.pulse2.write_register(2, data);},
            // no 0xA003
            0xB000 => {self.sawtooth.write_register(0, data);},
            0xB001 => {self.sawtooth.write_register(1, data);},
            0xB002 => {self.sawtooth.write_register(2, data);},
            0xB003 => {
                self.ppu_banking_mode = data & 0b0000_0011;
                self.mirroring_mode = (data & 0b0000_1100) >> 2;
                self.b003_shadow = data; // used for weird A10 nametable truth table
                self.nametable_chrrom = (data & 0b0001_0000) != 0;
                self.chr_a10_rules = (data & 0b0010_0000) != 0;
                self.prg_ram_enable = (data & 0b1000_0000) != 0;
            },
            0xC000 ..= 0xC003 => {
                self.prg_bank_8 = data as usize & 0x1F;
            },
            0xD000 => { self.r[0] = data as usize; },
            0xD001 => { self.r[1] = data as usize; },
            0xD002 => { self.r[2] = data as usize; },
            0xD003 => { self.r[3] = data as usize; },
            0xE000 => { self.r[4] = data as usize; },
            0xE001 => { self.r[5] = data as usize; },
            0xE002 => { self.r[6] = data as usize; },
            0xE003 => { self.r[7] = data as usize; },
            0xF000 => { self.irq_latch = data; },
            0xF001 => {
                self.irq_scanline_mode = ((data & 0b0000_0100) >> 2) == 0;
                self.irq_enable = (data & 0b0000_0010) != 0;
                self.irq_enable_after_acknowledgement = (data & 0b0000_0001) != 0;

                // acknowledge the pending IRQ if there is one
                self.irq_pending = false;

                // If the enable bit is set, setup for the next IRQ immediately, otherwise
                // do nothing (we may already have one in flight)
                if self.irq_enable {
                    self.irq_counter = self.irq_latch;
                    self.irq_scanline_prescaler = 341;                    
                }

            },
            0xF002 => {
                self.irq_pending = false;
                self.irq_enable = self.irq_enable_after_acknowledgement;
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => {
                // CHR Bank Selection
                match self.ppu_banking_mode {
                    0 => self._chr_mode_0(address),
                    1 => self._chr_mode_1(address),
                    2 => self._chr_mode_23(address),
                    3 => self._chr_mode_23(address),
                    _ => None
                }
            },
            0x2000 ..= 0x3FFF => {
                if self.chr_a10_rules {
                    match self.ppu_banking_mode {
                        0 => self._mirroring_mode_0_read(address),
                        1 => self._mirroring_mode_1_read(address),
                        2 => self._mirroring_mode_2_read(address),
                        3 => self._mirroring_mode_3_read(address),
                        _ => {
                            //println!("Unimplemented mirroring mode {}! Bailing.", self.ppu_banking_mode);
                            None
                        }
                    }
                } else {
                    // A10 rules weirdness
                    return self._a10_nametable_read(address);
                }
            }
            _ => None
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        match address {
            0x2000 ..= 0x3FFF => {
                if self.chr_a10_rules {
                    match self.ppu_banking_mode {
                        0 => self._mirroring_mode_0_write(address, data),
                        1 => self._mirroring_mode_1_write(address, data),
                        2 => self._mirroring_mode_2_write(address, data),
                        3 => self._mirroring_mode_3_write(address, data),
                        _ => {
                            //println!("Unimplemented mirroring mode {}! Bailing.", self.ppu_banking_mode);
                        }
                    }
                } else {
                    // A10 rules weirdness
                    self._a10_nametable_write(address, data);
                }
            }
            _ => {}
        }
    }

    fn channels(&self) ->  Vec<& dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        channels.push(&self.pulse1);
        channels.push(&self.pulse2);
        channels.push(&self.sawtooth);
        return channels;
    }

    fn channels_mut(&mut self) ->  Vec<&mut dyn AudioChannelState> {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        channels.push(&mut self.pulse1);
        channels.push(&mut self.pulse2);
        channels.push(&mut self.sawtooth);
        return channels;
    }

    fn record_expansion_audio_output(&mut self, _nes_sample: f32) {
        self.pulse1.record_current_output();
        self.pulse2.record_current_output();
        self.sawtooth.record_current_output();
    }
}
