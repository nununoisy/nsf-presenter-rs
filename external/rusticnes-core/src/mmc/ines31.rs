// iNES Mapper 031 represents a mapper created to facilitate cartridge compilations 
// of NSF music. It implements a common subset of the features used by NSFs. 
// Reference capabilities: https://wiki.nesdev.com/w/index.php/INES_Mapper_031

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct INes31 {
    pub prg_rom: MemoryBlock,
    pub chr: MemoryBlock,
    pub mirroring: Mirroring,
    pub vram: Vec<u8>,
    pub prg_banks: Vec<usize>,
}

impl INes31 {
    pub fn from_ines(ines: INesCartridge) -> Result<INes31, String> {
        let prg_rom_block = ines.prg_rom_block();
        let chr_block = ines.chr_block()?;

        return Ok(INes31 {
            prg_rom: prg_rom_block.clone(),
            chr: chr_block.clone(),
            mirroring: ines.header.mirroring(),
            vram: vec![0u8; 0x1000],
            prg_banks: vec![255usize; 8],
        })
    }
}

impl Mapper for INes31 {
    fn print_debug_status(&self) {
        println!("======= iNes 31 =======");
        println!("Mirroring Mode: {}", mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }
    
    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x8000 ..= 0x8FFF => self.prg_rom.banked_read(0x1000, self.prg_banks[0], (address as usize) - 0x8000),
            0x9000 ..= 0x9FFF => self.prg_rom.banked_read(0x1000, self.prg_banks[1], (address as usize) - 0x9000),
            0xA000 ..= 0xAFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[2], (address as usize) - 0xA000),
            0xB000 ..= 0xBFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[3], (address as usize) - 0xB000),
            0xC000 ..= 0xCFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[4], (address as usize) - 0xC000),
            0xD000 ..= 0xDFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[5], (address as usize) - 0xD000),
            0xE000 ..= 0xEFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[6], (address as usize) - 0xE000),
            0xF000 ..= 0xFFFF => self.prg_rom.banked_read(0x1000, self.prg_banks[7], (address as usize) - 0xF000),
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x5FF8 => {self.prg_banks[0] = data as usize},
            0x5FF9 => {self.prg_banks[1] = data as usize},
            0x5FFA => {self.prg_banks[2] = data as usize},
            0x5FFB => {self.prg_banks[3] = data as usize},
            0x5FFC => {self.prg_banks[4] = data as usize},
            0x5FFD => {self.prg_banks[5] = data as usize},
            0x5FFE => {self.prg_banks[6] = data as usize},
            0x5FFF => {self.prg_banks[7] = data as usize},
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
            0x0000 ..= 0x1FFF => self.chr.wrapping_write(address as usize, data),
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
