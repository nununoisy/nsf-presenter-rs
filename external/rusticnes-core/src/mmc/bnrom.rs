// BNROM, bank switchable PRG ROM, 8kb CHR RAM, solder-pad fixed horizontal or vertical mirroring.
// Essentially an AxROM variant, though I'm choosing to keep all numbered mapper implementations 
// dependency free for my own sanity.
// Reference capabilities: https://wiki.nesdev.com/w/index.php/BNROM

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct BnRom {
    pub prg_rom: MemoryBlock,
    pub chr: MemoryBlock,
    pub mirroring: Mirroring,
    pub prg_bank: usize,
    pub vram: Vec<u8>,
}

impl BnRom {
    pub fn from_ines(ines: INesCartridge) -> Result<BnRom, String> {
        let prg_rom_block = ines.prg_rom_block();
        let chr_block = ines.chr_block()?;

        return Ok(BnRom {
            prg_rom: prg_rom_block.clone(),
            chr: chr_block.clone(),
            mirroring: ines.header.mirroring(),
            prg_bank: 0x07,
            vram: vec![0u8; 0x1000],
        });
    }
}

impl Mapper for BnRom {
    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn print_debug_status(&self) {
        println!("======= BNROM =======");
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
            0x8000 ..= 0xFFFF => {self.prg_bank = data as usize;}
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => self.chr.wrapping_read(address as usize),
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
            0x0000 ..= 0x1FFF => {self.chr.wrapping_write(address as usize, data);},
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
