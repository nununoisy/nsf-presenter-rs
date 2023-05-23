// MMC2, a somewhat advanced bank switcher with extended CHR memory
// https://wiki.nesdev.com/w/index.php/MMC2

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct PxRom {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,
    pub mirroring: Mirroring,
    pub chr_0_latch: u8,
    pub chr_0_fd_bank: usize,
    pub chr_0_fe_bank: usize,
    pub chr_1_latch: u8,
    pub chr_1_fd_bank: usize,
    pub chr_1_fe_bank: usize,
    pub prg_bank: usize,
    pub vram: Vec<u8>,
}

impl PxRom {
    pub fn from_ines(ines: INesCartridge) -> Result<PxRom, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(PxRom {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            mirroring: Mirroring::Vertical,
            chr_0_latch: 0,
            chr_0_fd_bank: 0,
            chr_0_fe_bank: 0,
            chr_1_latch: 0,
            chr_1_fd_bank: 0,
            chr_1_fe_bank: 0,
            prg_bank: 0,
            vram: vec![0u8; 0x1000],
        })
    }
}

impl Mapper for PxRom {
    fn print_debug_status(&self) {
        println!("======= PxROM =======");
        println!("PRG Bank: {}, ", self.prg_bank);
        println!("CHR0 0xFD Bank: {}. CHR0 0xFE Bank: {}", self.chr_0_fd_bank, self.chr_0_fe_bank);
        println!("CHR1 0xFD Bank: {}. CHR1 0xFE Bank: {}", self.chr_1_fd_bank, self.chr_1_fe_bank);
        println!("Mirroring Mode: {}", mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }
  
    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x6000 ..= 0x7FFF => self.prg_ram.wrapping_read((address - 0x6000) as usize),
            0x8000 ..= 0x9FFF => self.prg_rom.banked_read(0x2000, self.prg_bank, address as usize - 0x8000),
            0xA000 ..= 0xBFFF => self.prg_rom.banked_read(0x2000, 0xFD,          address as usize - 0xA000),
            0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, 0xFE,          address as usize - 0xC000),
            0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF,          address as usize - 0xE000),
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            0x6000 ..= 0x7FFF => self.prg_ram.wrapping_write(address as usize, data),
            0xA000 ..= 0xAFFF => { self.prg_bank = (data & 0b0000_1111) as usize; },
            0xB000 ..= 0xBFFF => { self.chr_0_fd_bank = (data & 0b0001_1111) as usize; },
            0xC000 ..= 0xCFFF => { self.chr_0_fe_bank = (data & 0b0001_1111) as usize; },
            0xD000 ..= 0xDFFF => { self.chr_1_fd_bank = (data & 0b0001_1111) as usize; },
            0xE000 ..= 0xEFFF => { self.chr_1_fe_bank = (data & 0b0001_1111) as usize; },
            0xF000 ..= 0xFFFF => { 
                if data & 0b1 == 0 {
                    self.mirroring = Mirroring::Vertical;
                } else {
                    self.mirroring = Mirroring::Horizontal;
                }
            },
            _ => {}
        }
    }

    fn read_ppu(&mut self, address: u16) -> Option<u8> {
        match address {
            0x0FD8 => {self.chr_0_latch = 0;},
            0x0FE8 => {self.chr_0_latch = 1;},
            0x1FD8 ..= 0x1FDF => {self.chr_1_latch = 0;},
            0x1FE8 ..= 0x1FEF => {self.chr_1_latch = 1;},
            _ => {}
        }
        return self.debug_read_ppu(address);
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x0FFF => {
                let chr_bank = match self.chr_0_latch {
                    0 => self.chr_0_fd_bank,
                    1 => self.chr_0_fe_bank,
                    _ => 0
                };
                self.chr.banked_read(0x1000, chr_bank, address as usize - 0x0000)
            },
            0x1000 ..= 0x1FFF => {
                let chr_bank = match self.chr_1_latch {
                    0 => self.chr_1_fd_bank,
                    1 => self.chr_1_fe_bank,
                    _ => 0
                };
                self.chr.banked_read(0x1000, chr_bank, address as usize - 0x0000)
            },
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
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }
}
