// A very simple Mapper with no esoteric features or bank switching.
// Reference capabilities: https://wiki.nesdev.com/w/index.php/NROM

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct Action53 {
    prg_rom: MemoryBlock,
    prg_ram: MemoryBlock,
    chr: MemoryBlock,
    vram: Vec<u8>,

    register_select: u8,
    mirroring_mode: u8,
    chr_ram_a13_a14: usize,
    prg_inner_bank: usize,
    prg_outer_bank: usize,
    prg_mode: u8,
    prg_outer_bank_size: usize,
}

impl Action53 {
    pub fn from_ines(ines: INesCartridge) -> Result<Action53, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Action53 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            vram: vec![0u8; 0x1000],
            register_select: 0,
            mirroring_mode: 0,
            chr_ram_a13_a14: 0,
            prg_inner_bank: 0xFF,
            prg_outer_bank: 0xFF,
            prg_mode: 0,
            prg_outer_bank_size: 0,
        });
    }

    fn prg_address(&self, cpu_address: u16) -> usize {
        let cpu_a14 = ((cpu_address & 0b0100_0000_0000_0000) >> 14) as usize;
        let bits_a22_to_a14: usize = match self.prg_mode {
            0 | 1 => match self.prg_outer_bank_size {
                0 => (self.prg_outer_bank << 1) | cpu_a14,
                1 => ((self.prg_outer_bank & 0b1111_1110) << 1) | ((self.prg_inner_bank & 0b0000_0001) << 1) | cpu_a14,
                2 => ((self.prg_outer_bank & 0b1111_1100) << 1) | ((self.prg_inner_bank & 0b0000_0011) << 1) | cpu_a14,
                3 => ((self.prg_outer_bank & 0b1111_1000) << 1) | ((self.prg_inner_bank & 0b0000_0111) << 1) | cpu_a14,
                _ => 0 // unreachable
            },
            2 => match cpu_a14 {
                // $8000
                0 => self.prg_outer_bank << 1,
                // $C000
                1 => {
                    let outer_bitmask = (0b1_1111_1110 << self.prg_outer_bank_size) & 0b1_1111_1110;
                    let inner_bitmask = 0b0_0000_1111 >> (3 - self.prg_outer_bank_size);
                    ((self.prg_outer_bank << 1) & outer_bitmask) | (self.prg_inner_bank & inner_bitmask)
                },
                _ => 0 // unreachable
            },
            3 => match cpu_a14 {
                // $8000
                0 => {
                    let outer_bitmask = (0b1_1111_1110 << self.prg_outer_bank_size) & 0b1_1111_1110;
                    let inner_bitmask = 0b0_0000_1111 >> (3 - self.prg_outer_bank_size);
                    ((self.prg_outer_bank << 1) & outer_bitmask) | (self.prg_inner_bank & inner_bitmask)
                }
                // $C000
                1 => self.prg_outer_bank << 1 | 1,
                _ => 0 // unreachable
            },
            _ => 0 // unreachable
        };
        return (bits_a22_to_a14 << 14) | ((cpu_address & 0b0011_1111_1111_1111) as usize);
    }

    fn chr_address(&self, ppu_address: u16) -> usize {
        return (self.chr_ram_a13_a14 << 13) | ((ppu_address & 0b1_1111_1111_1111) as usize);
    }
}

impl Mapper for Action53 {
    fn mirroring(&self) -> Mirroring {
        match self.mirroring_mode {
            0 => Mirroring::OneScreenLower,
            1 => Mirroring::OneScreenUpper,
            2 => Mirroring::Vertical,
            3 => Mirroring::Horizontal,
            _ => Mirroring::Horizontal // unreachable
        }
    }
    
    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x6000 ..= 0x7FFF => {self.prg_ram.wrapping_read((address - 0x6000) as usize)},
            0x8000 ..= 0xFFFF => {
                self.prg_rom.wrapping_read(self.prg_address(address))
            },
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x5000 ..= 0x5FFF => {self.register_select = data & 0x81;},
            0x6000 ..= 0x7FFF => {self.prg_ram.wrapping_write((address - 0x6000) as usize, data);},
            0x8000 ..= 0xFFFF => {
                match self.register_select {
                    0x00 => {
                        if (self.mirroring_mode & 0b10) == 0 {
                            let mirroring_mode_bit_0: u8 = (data & 0b0001_0000) >> 4;
                            self.mirroring_mode = (self.mirroring_mode & 0b10) | mirroring_mode_bit_0;
                        }
                        self.chr_ram_a13_a14 = (data & 0b0000_0011) as usize;
                    },
                    0x01 => {
                        if (self.mirroring_mode & 0b10) == 0 {
                            let mirroring_mode_bit_0: u8 = (data & 0b0001_0000) >> 4;
                            self.mirroring_mode = (self.mirroring_mode & 0b10) | mirroring_mode_bit_0;
                        }
                        self.prg_inner_bank = (data & 0b0000_1111) as usize;
                    },
                    0x80 => {
                        self.mirroring_mode = data & 0b0000_0011;
                        self.prg_mode = (data & 0b0000_1100) >> 2;
                        self.prg_outer_bank_size = ((data & 0b0011_0000) >> 4) as usize;
                    },
                    0x81 => {
                        self.prg_outer_bank = data as usize;
                    },
                    _ => {/* never reached */}
                }
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => {return self.chr.wrapping_read(self.chr_address(address))},
            0x2000 ..= 0x3FFF => return match self.mirroring() {
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
            0x0000 ..= 0x1FFF => {self.chr.wrapping_write(self.chr_address(address), data);},
            0x2000 ..= 0x3FFF => match self.mirroring() {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                Mirroring::OneScreenLower => self.vram[mirroring::one_screen_lower(address) as usize] = data,
                Mirroring::OneScreenUpper => self.vram[mirroring::one_screen_upper(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
