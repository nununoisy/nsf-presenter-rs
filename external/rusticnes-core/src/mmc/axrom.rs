// AxROM, bank switchable PRG ROM, 8kb CHR RAM, basic single-screen mirroring.
// Reference capabilities: https://wiki.nesdev.com/w/index.php/AxROM

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct AxRom {
    pub prg_rom: MemoryBlock,
    pub chr: MemoryBlock,
    pub mirroring: Mirroring,
    pub prg_bank: usize,
    pub vram: Vec<u8>,
}

impl AxRom {
    pub fn from_ines(ines: INesCartridge) -> Result<AxRom, String> {
        let prg_rom_block = ines.prg_rom_block();
        let chr_block = ines.chr_block()?;

        return Ok(AxRom {
            prg_rom: prg_rom_block.clone(),
            chr: chr_block.clone(),
            mirroring: Mirroring::OneScreenUpper,
            prg_bank: 0x07,
            vram: vec![0u8; 0x1000],
        });
    }
}

impl Mapper for AxRom {
    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn print_debug_status(&self) {
        println!("======= AxROM =======");
        println!("PRG Bank: {}, Mirroring Mode: {}", self.prg_bank, mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x8000 ..= 0xFFFF => {self.prg_rom.banked_read(0x8000, self.prg_bank, (address - 0x8000) as usize)},
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x8000 ..= 0xFFFF => {
                self.prg_bank = (data & 0x07) as usize;
                if data & 0x10 == 0 {
                    self.mirroring = Mirroring::OneScreenLower;
                } else {
                    self.mirroring = Mirroring::OneScreenUpper;
                }
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => self.chr.wrapping_read(address as usize),
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::OneScreenLower => Some(self.vram[mirroring::one_screen_lower(address) as usize]),
                Mirroring::OneScreenUpper => Some(self.vram[mirroring::one_screen_upper(address) as usize]),
                _ => None
            },
            _ => None
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        match address {
            0x0000 ..= 0x1FFF => self.chr.wrapping_write(address as usize, data),
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::OneScreenLower => self.vram[mirroring::one_screen_lower(address) as usize] = data,
                Mirroring::OneScreenUpper => self.vram[mirroring::one_screen_upper(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
