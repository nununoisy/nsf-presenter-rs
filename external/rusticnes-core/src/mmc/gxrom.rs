// GxRom, simple bank switchable 32kb PRG ROM and 8k CHR ROM
// Reference capabilities: https://wiki.nesdev.com/w/index.php/GxROM

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct GxRom {
    pub prg_rom: MemoryBlock,
    pub chr: MemoryBlock,
    pub mirroring: Mirroring,
    pub prg_bank: usize,
    pub chr_bank: usize,
    pub vram: Vec<u8>,
}

impl GxRom {
    pub fn from_ines(ines: INesCartridge) -> Result<GxRom, String> {
        let prg_rom_block = ines.prg_rom_block();
        let chr_block = ines.chr_block()?;

        return Ok(GxRom {
            prg_rom: prg_rom_block.clone(),
            chr: chr_block.clone(),
            mirroring: ines.header.mirroring(),
            prg_bank: 0x00,
            chr_bank: 0x00,
            vram: vec![0u8; 0x1000],
        });
    }
}

impl Mapper for GxRom {
    fn print_debug_status(&self) {
        println!("======= GxROM =======");
        println!("PRG Bank: {}, CHR Bank: {}, Mirroring Mode: {}", self.prg_bank, self.chr_bank, mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
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
                self.prg_bank = ((data & 0b0011_0000) >> 4) as usize;
                self.chr_bank =  (data & 0b0000_0011) as usize;
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => self.chr.banked_read(0x2000, self.chr_bank, address as usize),
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                Mirroring::Vertical   => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                _ => None
            },
            _ => None
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        match address {
            0x0000 ..= 0x1FFF => self.chr.banked_write(0x2000, self.chr_bank, address as usize, data),
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
