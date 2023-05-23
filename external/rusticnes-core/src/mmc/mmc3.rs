// Advanced mapper with bank-switched PRG ROM and CHR ROM, and a scanline counter feeding into IRQ
// Reference capabilities: https://wiki.nesdev.com/w/index.php/MMC3

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct Mmc3 {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,
    pub vram: Vec<u8>,

    pub chr2_bank_0: usize,
    pub chr2_bank_1: usize,
    pub chr1_bank_2: usize,
    pub chr1_bank_3: usize,
    pub chr1_bank_4: usize,
    pub chr1_bank_5: usize,

    pub prg_bank_6: usize,
    pub prg_bank_7: usize,

    pub switch_chr_banks: bool,
    pub switch_prg_banks: bool,

    pub bank_select: u8,

    pub irq_counter: u8,
    pub irq_reload: u8,
    pub irq_reload_requested: bool,
    pub irq_enabled: bool,
    pub irq_flag: bool,

    pub last_a12: u8,
    pub filtered_a12: u8,
    pub low_a12_counter: u8,

    // Debug
    pub last_chr_read: u16,

    pub mirroring: Mirroring,
}

impl Mmc3 {
    pub fn from_ines(ines: INesCartridge) -> Result<Mmc3, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Mmc3 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            vram: vec![0u8; 0x2000],
            // Note: On real MMC3-based hardware, many of these values are random on startup, so
            // the defaults presented below are arbitrary.
            chr2_bank_0: 0,
            chr2_bank_1: 0,
            chr1_bank_2: 0,
            chr1_bank_3: 0,
            chr1_bank_4: 0,
            chr1_bank_5: 0,

            prg_bank_6: 0,
            prg_bank_7: 0,

            switch_chr_banks: false,
            switch_prg_banks: false,

            bank_select: 0,

            irq_counter: 0,
            irq_reload: 0,
            irq_reload_requested: false,
            irq_enabled: false,
            irq_flag: false,

            last_a12: 0,
            filtered_a12: 0,
            last_chr_read: 0,
            low_a12_counter: 0,

            mirroring: ines.header.mirroring(),
        })
    }

    fn snoop_ppu_a12(&mut self, address: u16) {
        self.last_chr_read = address;
        let current_a12 = ((address & 0b0001_0000_0000_0000) >> 12) as u8;
        
        let last_filtered_a12 = self.filtered_a12;

        if current_a12 == 1 {     
            self.filtered_a12 = 1;
            self.low_a12_counter = 0;
        }

        let filtered_a12_rising_edge = (self.filtered_a12 == 1) && (last_filtered_a12 == 0);
        if filtered_a12_rising_edge {
            self.clock_irq_counter();
        }

        // Caching this value so the M2 counter can see it
        self.last_a12 = current_a12;
    }

    fn snoop_cpu_m2(&mut self) {
        if self.low_a12_counter < 255 && self.last_a12 == 0 {
            self.low_a12_counter += 1;
        }
        if self.low_a12_counter >= 3 {            
            self.filtered_a12 = 0;
        }
    }

    fn clock_irq_counter(&mut self) {
        if self.irq_counter == 0 || self.irq_reload_requested {
            self.irq_counter = self.irq_reload;
            self.irq_reload_requested = false;
        } else {
            self.irq_counter -= 1;                        
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_flag = true;                        
        }
    }

    fn _read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            // CHR
            0x0000 ..= 0x1FFF => {
                if self.switch_chr_banks {
                    match address {
                        0x0000 ..= 0x03FF => self.chr.banked_read(0x400, self.chr1_bank_2, address as usize -  0x000),
                        0x0400 ..= 0x07FF => self.chr.banked_read(0x400, self.chr1_bank_3, address as usize -  0x400),
                        0x0800 ..= 0x0BFF => self.chr.banked_read(0x400, self.chr1_bank_4, address as usize -  0x800),
                        0x0C00 ..= 0x0FFF => self.chr.banked_read(0x400, self.chr1_bank_5, address as usize -  0xC00),
                        0x1000 ..= 0x17FF => self.chr.banked_read(0x800, self.chr2_bank_0 >> 1, address as usize - 0x1000),
                        0x1800 ..= 0x1FFF => self.chr.banked_read(0x800, self.chr2_bank_1 >> 1, address as usize - 0x1800),
                        _ => None,
                    }
                } else {
                    match address {
                        0x0000 ..= 0x07FF => self.chr.banked_read(0x800, self.chr2_bank_0 >> 1, address as usize -  0x000),
                        0x0800 ..= 0x0FFF => self.chr.banked_read(0x800, self.chr2_bank_1 >> 1, address as usize -  0x800),
                        0x1000 ..= 0x13FF => self.chr.banked_read(0x400, self.chr1_bank_2, address as usize - 0x1000),
                        0x1400 ..= 0x17FF => self.chr.banked_read(0x400, self.chr1_bank_3, address as usize - 0x1400),
                        0x1800 ..= 0x1BFF => self.chr.banked_read(0x400, self.chr1_bank_4, address as usize - 0x1800),
                        0x1C00 ..= 0x1FFF => self.chr.banked_read(0x400, self.chr1_bank_5, address as usize - 0x1C00),
                        _ => None,
                    }
                }
            },
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                Mirroring::Vertical   => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                Mirroring::FourScreen => Some(self.vram[mirroring::four_banks(address) as usize]),
                _ => None
            },
            _ => None
        }
    }
}

impl Mapper for Mmc3 {
    fn print_debug_status(&self) {
        println!("======= MMC3 =======");
        println!("IRQ: Current: {}, Reload: {}", self.irq_counter, self.irq_reload);
        println!("Last A12: {}, Last CHR Read: 0x{:04X}", self.last_a12, self.last_chr_read);
        println!("Mirroring Mode: {}", mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn irq_flag(&self) -> bool {
        return self.irq_flag;
    }

    fn clock_cpu(&mut self) {
        self.snoop_cpu_m2();
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            // PRG RAM
            0x6000 ..= 0x7FFF => {
                self.prg_ram.wrapping_read(address as usize - 0x6000)
            },
            // PRG ROM
            0x8000 ..= 0xFFFF => {
                if self.switch_prg_banks {
                    match address {
                        0x8000 ..= 0x9FFF => self.prg_rom.banked_read(0x2000, 0xFE,            address as usize -  0x8000),
                        0xA000 ..= 0xBFFF => self.prg_rom.banked_read(0x2000, self.prg_bank_7, address as usize -  0xA000),
                        0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, self.prg_bank_6, address as usize -  0xC000),
                        0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF,            address as usize -  0xE000),
                        _ => None,
                    }
                } else {
                    match address {
                        0x8000 ..= 0x9FFF => self.prg_rom.banked_read(0x2000, self.prg_bank_6, address as usize -  0x8000),
                        0xA000 ..= 0xBFFF => self.prg_rom.banked_read(0x2000, self.prg_bank_7, address as usize -  0xA000),
                        0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, 0xFE,            address as usize -  0xC000),
                        0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF,            address as usize -  0xE000),
                        _ => None,
                    }
                }
            },
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            // PRG RAM
            0x6000 ..= 0x7FFF => {
                // Note: Intentionally omitting PRG RAM protection feature, since this
                // retains compatability with assumptions about iNES mapper 004
                self.prg_ram.wrapping_write(address as usize - 0x6000, data)
            },
            // Registers
            0x8000 ..= 0xFFFF => {
                if address & 0b1 == 0 {
                    // Even Registers
                    match address {
                        0x8000 ..= 0x9FFF => {
                            // Bank Select
                            self.bank_select =      data & 0b0000_0111;
                            self.switch_prg_banks = (data & 0b0100_0000) != 0;
                            self.switch_chr_banks = (data & 0b1000_0000) != 0;
                        },
                        0xA000 ..= 0xBFFF => {
                            if self.mirroring != Mirroring::FourScreen {
                                if data & 0b1 == 0 {
                                    self.mirroring = Mirroring::Vertical;
                                } else {
                                    self.mirroring = Mirroring::Horizontal;
                                }
                            }
                        },
                        0xC000 ..= 0xDFFF => {
                            self.irq_reload = data;
                        },
                        0xE000 ..= 0xFFFF => {
                            self.irq_enabled = false;
                            self.irq_flag = false;
                        }

                        _ => (),
                    }
                } else {
                    // Odd Registers
                    match address {
                        0x8000 ..= 0x9FFF => {
                            // Bank Data
                            match self.bank_select {
                                0 => self.chr2_bank_0 = (data & 0b1111_1110) as usize,
                                1 => self.chr2_bank_1 = (data & 0b1111_1110) as usize,
                                2 => self.chr1_bank_2 = data as usize,
                                3 => self.chr1_bank_3 = data as usize,
                                4 => self.chr1_bank_4 = data as usize,
                                5 => self.chr1_bank_5 = data as usize,
                                6 => self.prg_bank_6  = (data & 0b0011_1111) as usize,
                                7 => self.prg_bank_7  = (data & 0b0011_1111) as usize,
                                _ => (),
                            }
                        },
                        0xA000 ..= 0xBFFF => {
                            // PRG RAM Protect
                            // Intentionally not emulated, for compatability with iNES mapper 004
                        },
                        0xC000 ..= 0xDFFF => {
                            self.irq_reload_requested = true;
                        },
                        0xE000 ..= 0xFFFF => {
                            self.irq_enabled = true;
                        }
                        _ => (),
                    }
                }
            },
            _ => (),
        }
    }

    fn read_ppu(&mut self, address: u16) -> Option<u8> {
        self.snoop_ppu_a12(address);
        return self._read_ppu(address);
    }

    fn access_ppu(&mut self, address: u16) {
        self.snoop_ppu_a12(address);
    }    

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        return self._read_ppu(address);
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        self.snoop_ppu_a12(address);
        match address {
            // CHR RAM (if enabled)
            0x0000 ..= 0x1FFF => {
                self.last_chr_read = address;
                let current_a12 = ((address & 0b0001_0000_0000_0000) >> 12) as u8;
                if current_a12 == 1 && self.last_a12 == 0 {
                    if self.irq_counter == 0 || self.irq_reload_requested {
                        self.irq_counter = self.irq_reload;
                        self.irq_reload_requested = false;
                    } else {
                        self.irq_counter -= 1;                        
                    }
                    if self.irq_counter == 0 && self.irq_enabled {
                        self.irq_flag = true;                        
                    }
                }
                self.last_a12 = current_a12;
                if self.switch_chr_banks {
                    match address {
                        0x0000 ..= 0x03FF => self.chr.banked_write(0x400, self.chr1_bank_2, address as usize -  0x000, data),
                        0x0400 ..= 0x07FF => self.chr.banked_write(0x400, self.chr1_bank_3, address as usize -  0x400, data),
                        0x0800 ..= 0x0BFF => self.chr.banked_write(0x400, self.chr1_bank_4, address as usize -  0x800, data),
                        0x0C00 ..= 0x0FFF => self.chr.banked_write(0x400, self.chr1_bank_5, address as usize -  0xC00, data),
                        0x1000 ..= 0x17FF => self.chr.banked_write(0x800, self.chr2_bank_0 >> 1, address as usize - 0x1000, data),
                        0x1800 ..= 0x1FFF => self.chr.banked_write(0x800, self.chr2_bank_1 >> 1, address as usize - 0x1800, data),
                        _ => {},
                    }
                } else {
                    match address {
                        0x0000 ..= 0x07FF => self.chr.banked_write(0x800, self.chr2_bank_0 >> 1, address as usize -  0x000, data),
                        0x0800 ..= 0x0FFF => self.chr.banked_write(0x800, self.chr2_bank_1 >> 1, address as usize -  0x800, data),
                        0x1000 ..= 0x13FF => self.chr.banked_write(0x400, self.chr1_bank_2, address as usize - 0x1000, data),
                        0x1400 ..= 0x17FF => self.chr.banked_write(0x400, self.chr1_bank_3, address as usize - 0x1400, data),
                        0x1800 ..= 0x1BFF => self.chr.banked_write(0x400, self.chr1_bank_4, address as usize - 0x1800, data),
                        0x1C00 ..= 0x1FFF => self.chr.banked_write(0x400, self.chr1_bank_5, address as usize - 0x1C00, data),
                        _ => {},
                    }
                }
            },
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                Mirroring::FourScreen => self.vram[mirroring::four_banks(address) as usize] = data,
                _ => {}
            },
            _ => (),
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
}
