// Common mapper with bank switched PRG_ROM, CHR_ROM/RAM, and optional PRG RAM.
// Reference capabilities: https://wiki.nesdev.com/w/index.php/MMC1

use ines::INesCartridge;
use memoryblock::MemoryBlock;

use mmc::mapper::*;
use mmc::mirroring;

pub struct Mmc1 {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,
    pub vram: Vec<u8>,

    pub shift_counter: u8,
    pub shift_data: u8,

    pub chr_bank_0: usize,
    pub chr_bank_1: usize,

    pub prg_bank: usize,
    pub prg_ram_enabled: bool,
    pub prg_ram_bank: usize,

    pub control: u8,

    pub mirroring: Mirroring,
    pub last_write: bool,
}

impl Mmc1 {
    pub fn from_ines(ines: INesCartridge) -> Result<Mmc1, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Mmc1 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            vram: vec![0u8; 0x1000],            
            // Note: On real MMC1-based hardware, many of these values are random on startup, so
            // the defaults presented below are arbitrary.
            shift_counter: 0,
            shift_data: 0,
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0x00,
            prg_ram_enabled: true,
            prg_ram_bank: 0,
            // Power-on in PRG mode 3, so the last bank is fixed and reset vectors are reliably available.
            // (Real hardware might not do this consistently?)
            control: 0x0C,
            mirroring: Mirroring::Vertical,
            last_write: false,
        })
    }
}

impl Mapper for Mmc1 {
    fn print_debug_status(&self) {
        let prg_mode = (self.control >> 2) & 0x3;
        let chr_mode = (self.control & 0x10) >> 4;
        println!("======= MMC1 =======");
        println!("PRG Mode: {} | CHR: Mode: {} | S.Count: {} | S.Data: {:02X}",
            prg_mode, chr_mode, self.shift_counter, self.shift_data);
        let last_bank = (self.prg_rom.len() / (16 * 1024)) as u16 - 1;
        println!("PRG: {} | CHR0: {} | CHR1: {} | PRG_LAST: {}",
            self.prg_bank, self.chr_bank_0, self.chr_bank_1, last_bank);
        println!("Mirroring Mode: {}", mirroring_mode_name(self.mirroring));
        println!("====================");
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn read_cpu(&mut self, address: u16) -> Option<u8> {
        self.last_write = false;
        return self.debug_read_cpu(address);
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            // PRG RAM
            0x6000 ..= 0x7FFF => {
                self.prg_ram.banked_read(0x2000, self.prg_ram_bank, address as usize)
            },
            // PRG ROM - First 16k Page
            0x8000 ..= 0xBFFF => {
                let prg_rom_len = self.prg_rom.len();
                if prg_rom_len > 0 {
                    let prg_mode = (self.control >> 2) & 0x3;
                    match prg_mode {
                        0 | 1 => {
                            // 32kb PRG mode, use prg_bank ignoring bit 0
                            let lower_half_bank = self.prg_bank & 0xFFFE;
                            return self.prg_rom.banked_read(0x4000, lower_half_bank, (address - 0x8000) as usize)
                        },
                        2 => {
                            // Fixed first bank, read that out here
                            return self.prg_rom.banked_read(0x4000, 0, (address - 0x8000) as usize)
                        },
                        3 => {
                            // Fixed last bank, read out the bank-switched first bank
                            return self.prg_rom.banked_read(0x4000, self.prg_bank, (address - 0x8000) as usize)
                        },
                        _ => return None, // Never called
                    }
                } else {
                    return None;
                }
            },
            // PRG ROM - Last 16k Page
            0xC000 ..= 0xFFFF => {
                let prg_rom_len = self.prg_rom.len();
                if prg_rom_len > 0 {
                    let prg_mode = (self.control >> 2) & 0x3;
                    match prg_mode {
                        0 | 1 => {
                            // 32kb PRG mode, use prg_bank and force-set bit 1
                            let upper_half_bank = self.prg_bank | 0x0001;
                            return self.prg_rom.banked_read(0x4000, upper_half_bank, (address - 0x8000) as usize)
                        },
                        2 => {
                            // Fixed first bank, read out the bank-switched second bank
                            return self.prg_rom.banked_read(0x4000, self.prg_bank, (address - 0x8000) as usize)
                        },
                        3 => {
                            // Fixed last bank, read out the bank-switched *last* bank
                            return self.prg_rom.banked_read(0x4000, 0xFF, (address - 0x8000) as usize)
                        },
                        _ => return None, // Never called
                    }
                } else {
                    return None;
                }
            },
            _ => return None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            // PRG RAM
            0x6000 ..= 0x7FFF => {
                if self.prg_ram_enabled {
                    self.prg_ram.banked_write(0x2000, self.prg_ram_bank, address as usize, data);
                }
            },
            // Control Registers
            0x8000 ..= 0xFFFF => {
                if self.last_write {
                    // Ignore this write! MMC1 ignores successive writes, and will clear this flag
                    // on the next read cycle.
                    return;
                }
                self.last_write = true;
                
                if data & 0x80 != 0 {
                    // Shift / Control Reset!
                    self.shift_counter = 0;
                    // Upon reset, this sets the PRG ROM mode to 3, which fixes the last bank
                    // to the upper PRG Page. This is the startup state of MMC1 variants.
                    // https://wiki.nesdev.com/w/index.php/MMC1#Load_register_.28.248000-.24FFFF.29
                    self.control = self.control | 0b0_1100;
                } else {
                    self.shift_data = (self.shift_data >> 1) | ((data & 0b1) << 4);
                    self.shift_counter += 1;
                    if self.shift_counter == 5 {
                        // Only the top 3 bits (13-15) matter for register selection, everything
                        // else is mirrored due to incomplete decoding of the address.
                        // https://wiki.nesdev.com/w/index.php/MMC1#Registers
                        let register = address & 0b1110_0000_0000_0000;
                        match register {
                            0x8000 ..= 0x9F00 => {
                                self.control = self.shift_data;
                                let nametable_mode = self.control & 0b0_0011;
                                match nametable_mode {
                                    0 => self.mirroring = Mirroring::OneScreenLower,
                                    1 => self.mirroring = Mirroring::OneScreenUpper,
                                    2 => self.mirroring = Mirroring::Vertical,
                                    3 => self.mirroring = Mirroring::Horizontal,
                                    _ => println!("Bad mirroring mode!! {}", nametable_mode),
                                }
                            },
                            0xA000 ..= 0xBF00 => {
                                self.chr_bank_0 = self.shift_data as usize;
                                self.prg_ram_bank = ((self.shift_data & 0b0_1100) >> 2) as usize;
                            },
                            0xC000 ..= 0xDF00 => {
                                self.chr_bank_1 = self.shift_data as usize;
                                self.prg_ram_bank = ((self.shift_data & 0b0_1100) >> 2) as usize;
                            },
                            0xE000 ..= 0xFF00 => {
                                // The 5th bit disables RAM, so invert it here to decide when
                                // RAM should be enabled.
                                // TODO: This is ignored on certain MMC variants!
                                self.prg_ram_enabled = self.shift_data & 0b1_0000 == 0;
                                self.prg_bank = (self.shift_data & 0b0_1111) as usize;
                            },
                            _ => ()
                        }
                        self.shift_counter = 0;
                        self.shift_data = 0;
                    }
                }
            }
            _ => {}
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            // CHR Bank 0
            0x0000 ..= 0x0FFF => {
                if self.control & 0x10 == 0 {
                    // 8kb CHR mode, bit 0 is treated as cleared
                    let lower_half_bank = self.chr_bank_0 & 0xFFFE;
                    return self.chr.banked_read(0x1000, lower_half_bank, address as usize)
                } else {
                    // 4kb CHR mode
                    return self.chr.banked_read(0x1000, self.chr_bank_0 , address as usize)
                }
            },
            // CHR Bank 1
            0x1000 ..= 0x1FFF => {
                if self.control & 0x10 == 0 {
                    // 8kb CHR mode, bit 0 is treated as set
                    let upper_half_bank = self.chr_bank_0 | 0x0001;
                    return self.chr.banked_read(0x1000, upper_half_bank, address as usize)
                } else {
                    // 4kb CHR mode
                    return self.chr.banked_read(0x1000, self.chr_bank_1 , address as usize)
                }
            },
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
            // CHR Bank 0
            0x0000 ..= 0x0FFF => {
                if self.control & 0x10 == 0 {
                    // 8kb CHR mode, bit 0 is ignored
                    let lower_half_bank = self.chr_bank_0 & 0xFFFE;
                    self.chr.banked_write(0x1000, lower_half_bank, address as usize, data)
                } else {
                    // 4kb CHR mode
                    self.chr.banked_write(0x1000, self.chr_bank_0, address as usize, data)
                }
            },
            // CHR Bank 1
            0x1000 ..= 0x1FFF => {
                if self.control & 0x10 == 0 {
                    // 8kb CHR mode, use chr_bank_0 with bit 1 set
                    let upper_half_bank = self.chr_bank_0 | 0x0001;
                    self.chr.banked_write(0x1000, upper_half_bank, address as usize, data)
                } else {
                    // 4kb CHR mode
                    self.chr.banked_write(0x1000, self.chr_bank_1, address as usize, data)
                }                
            },
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
}
