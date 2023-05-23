// Parses and manages header fields and data blobs for iNES 1.0 and 2.0
// Details here: https://wiki.nesdev.com/w/index.php/INES
// And here: https://wiki.nesdev.com/w/index.php/NES_2.0#Default_Expansion_Device
// iNES 2.0 is always preferred when detected.

use std::io::Read;
use std::error::Error;
use std::fmt;

use mmc::mapper::Mirroring;
use memoryblock::MemoryBlock;
use memoryblock::MemoryType;

#[derive(Debug)]
pub enum INesError {
    InvalidHeader,
    Unimplemented,
    ReadError{reason: String}
}

impl Error for INesError {}

impl fmt::Display for INesError  {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            INesError::InvalidHeader => {write!(f, "Invalid iNES Header")},
            INesError::Unimplemented => {write!(f, "Unimplemented (Lazy programmers!!1)")},
            INesError::ReadError{reason} => {write!(f, "Error reading cartridge: {}", reason)}
        }
    }
}

impl From<std::io::Error> for INesError {
    fn from(error: std::io::Error) -> Self {
        return INesError::ReadError{reason: error.to_string()};
    }
}

#[derive(Copy, Clone)]
pub struct INesHeader {
    raw_bytes: [u8; 16]
}

// header byte constants
const INES_MAGIC_N: usize = 0;
const INES_MAGIC_E: usize = 1;
const INES_MAGIC_S: usize = 2;
const INES_MAGIC_EOF: usize = 3;
const INES_PRG_ROM_LSB: usize = 4;
const INES_CHR_ROM_LSB: usize = 5;
const INES_FLAGS_6: usize = 6;
const INES_FLAGS_7: usize = 7;

// here the constants diverge depending on type
const INES1_PRG_RAM_SIZE: usize = 8;
//const INES1_TV_SYSTEM: usize = 9;
//const INES1_FLAGS_10: usize = 10;

const INES2_MAPPER_SUB_MSB: usize = 8;
const INES2_PRG_CHR_MSB: usize = 9;
const INES2_PRG_RAM: usize = 10;
const INES2_CHR_RAM: usize = 11;
//const INES2_CPU_PPU_TIMING: usize = 12;
//const INES2_SYSTEM_TYPE: usize = 13;
//const INES2_MISC_ROM_COUNT: usize = 14;
//const INES2_DEFAULT_EXPANSION: usize = 15;

impl INesHeader {
    pub fn from(raw_bytes: &[u8]) -> INesHeader {
        let mut header = INesHeader {
            raw_bytes: [0u8; 16],
        };
        header.raw_bytes.copy_from_slice(&raw_bytes[0..16]);
        return header;
    }

    pub fn magic_header_valid(&self) -> bool {
        // Constant $4E $45 $53 $1A ("NES" followed by MS-DOS end-of-file)
        return 
            self.raw_bytes[INES_MAGIC_N] as char == 'N' &&
            self.raw_bytes[INES_MAGIC_E] as char == 'E' &&
            self.raw_bytes[INES_MAGIC_S] as char == 'S' &&
            self.raw_bytes[INES_MAGIC_EOF] == 0x1A;
    }

    fn ines1_extended_attributes_valid(&self) -> bool {
        // Or in other words, "DiskDude!" is missing:
        return self.raw_bytes[12] | self.raw_bytes[13] | self.raw_bytes[14] | self.raw_bytes[15] == 0;
    }

    pub fn version(&self) -> u8 {
        // A file is a NES 2.0 ROM image file if it begins with "NES<EOF>" (same as iNES) and, 
        // additionally, the byte at offset 7 has bit 2 clear and bit 3 set:
        if (self.raw_bytes[INES_FLAGS_7] & 0x0C) == 0x08 {
            return 2;
        }
        if self.magic_header_valid() {
            return 1;
        }
        return 0;
    }

    fn _prg_size_ines1(&self) -> usize {
        return self.raw_bytes[INES_PRG_ROM_LSB] as usize * 16 * 1024;
    }

    fn _prg_size_ines2(&self) -> usize {
        // https://wiki.nesdev.com/w/index.php/NES_2.0#PRG-ROM_Area
        let lsb = self.raw_bytes[INES_PRG_ROM_LSB];
        let msb = self.raw_bytes[INES2_PRG_CHR_MSB] & 0b0000_1111;
        if msb == 0xF {
            // exponent-multiplier mode
            //  ++++----------- Header byte 9 D0..D3
            //  |||| ++++-++++- Header byte 4
            //  D~BA98 7654 3210
            //  --------------
            //  1111 EEEE EEMM
            //  |||| ||++- Multiplier, actual value is MM*2+1 (1,3,5,7)
            //  ++++-++--- Exponent (2^E), 0-63

            let multiplier = ((lsb & 0b0000_0011) * 2 + 1) as usize;
            let exponent = ((lsb & 0b1111_1100) >> 2) as u32;
            let base: usize = 2;
            return base.pow(exponent) * multiplier;
        } else {
            // simple mode
            return ((msb as usize) << 8) + (lsb as usize) * 16 * 1024;
        }
    }

    pub fn prg_size(&self) -> usize {
        return match self.version() {
            1 => self._prg_size_ines1(),
            2 => self._prg_size_ines2(),
            _ => 0
        }
    }

    fn _chr_rom_size_ines1(&self) -> usize {
        let chr_size = self.raw_bytes[INES_CHR_ROM_LSB] as usize * 8 * 1024;
        return chr_size;
    }

    fn _chr_rom_size_ines2(&self) -> usize {
        // https://wiki.nesdev.com/w/index.php/NES_2.0#PRG-ROM_Area
        let lsb = self.raw_bytes[INES_CHR_ROM_LSB];
        let msb = (self.raw_bytes[INES2_PRG_CHR_MSB] & 0b1111_0000) >> 4;
        if msb == 0xF {
            // exponent-multiplier mode
            //  ++++----------- Header byte 9 D0..D3
            //  |||| ++++-++++- Header byte 4
            //  D~BA98 7654 3210
            //  --------------
            //  1111 EEEE EEMM
            //  |||| ||++- Multiplier, actual value is MM*2+1 (1,3,5,7)
            //  ++++-++--- Exponent (2^E), 0-63

            let multiplier = ((lsb & 0b0000_0011) * 2 + 1) as usize;
            let exponent = ((lsb & 0b1111_1100) >> 2) as u32;
            let base: usize = 2;
            return base.pow(exponent) * multiplier;
        } else {
            // simple mode
            return ((msb as usize) << 8) + (lsb as usize) * 8 * 1024;
        }
    }

    pub fn chr_rom_size(&self) -> usize {
        return match self.version() {
            1 => self._chr_rom_size_ines1(),
            2 => self._chr_rom_size_ines2(),
            _ => 0
        }
    }

    fn _chr_ram_size_ines1(&self) -> usize {
        if self.raw_bytes[INES_CHR_ROM_LSB] == 0 {
            return 8 * 1024;
        }
        return 0;
    }

    fn _chr_ram_size_ines2(&self) -> usize {
        let shift_count = self.raw_bytes[INES2_CHR_RAM] & 0b0000_1111;
        if shift_count == 0 {
            return 0;
        }
        return 64 << (shift_count as usize);
    }

    pub fn chr_ram_size(&self) -> usize {
        return match self.version() {
            1 => self._chr_ram_size_ines1(),
            2 => self._chr_ram_size_ines2(),
            _ => 0
        }
    }

    pub fn chr_sram_size(&self) -> usize {
        // Note: iNes2.0 calls this NVRAM, we're going with SRAM to match
        // RusticNES's conventions, and also user expectation
        if self.version() != 2 {
            return 0;
        }
        let shift_count = (self.raw_bytes[INES2_CHR_RAM] & 0b1111_0000) >> 4;
        if shift_count == 0 {
            return 0;
        }
        return 64 << (shift_count as usize);
    }

    // https://wiki.nesdev.com/w/index.php/INES#Flags_6

    pub fn mirroring(&self) -> Mirroring {
        if self.raw_bytes[INES_FLAGS_6] & 0b0000_1000 != 0 {
            return Mirroring::FourScreen;
        }
        if self.raw_bytes[INES_FLAGS_6] & 0b0000_0001 != 0 {
            return Mirroring::Vertical;
        }
        return Mirroring::Horizontal;
    }

    pub fn has_sram(&self) -> bool {
        return self.raw_bytes[INES_FLAGS_6] & 0b0000_0010 != 0;
    }

    fn _prg_ram_size_ines1(&self) -> usize  {
        let has_sram = self.raw_bytes[INES_FLAGS_6] & 0b0000_0010 != 0;
        if has_sram {
            return 0;
        }
        if self.ines1_extended_attributes_valid() && self.raw_bytes[INES1_PRG_RAM_SIZE] != 0 {
            return (self.raw_bytes[INES1_PRG_RAM_SIZE] as usize) * 8 * 1024;
        } else {
            return 8 * 1024;
        }
    }

    fn _prg_ram_size_ines2(&self) -> usize  {
        let shift_count = self.raw_bytes[INES2_PRG_RAM] & 0b0000_1111;
        if shift_count == 0 {
            return 0;
        }
        return 64 << (shift_count as usize);
    }

    pub fn prg_ram_size(&self) -> usize  {
        return match self.version() {
            1 => self._prg_ram_size_ines1(),
            2 => self._prg_ram_size_ines2(),
            _ => 0
        }
    }

    fn _prg_sram_size_ines1(&self) -> usize  {
        let has_sram = self.raw_bytes[INES_FLAGS_6] & 0b0000_0010 != 0;
        if !has_sram {
            return 0;
        }
        if self.ines1_extended_attributes_valid() && self.raw_bytes[INES1_PRG_RAM_SIZE] != 0 {
            return (self.raw_bytes[INES1_PRG_RAM_SIZE] as usize) * 8 * 1024;
        } else {
            return 8 * 1024;
        }
    }

    fn _prg_sram_size_ines2(&self) -> usize {
        let shift_count = (self.raw_bytes[INES2_PRG_RAM] & 0b1111_0000) >> 4;
        if shift_count == 0 {
            return 0;
        }
        return 64 << (shift_count as usize);
    }

    pub fn prg_sram_size(&self) -> usize  {
        return match self.version() {
            1 => self._prg_sram_size_ines1(),
            2 => self._prg_sram_size_ines2(),
            _ => 0
        }   
    }
    
    pub fn has_trainer(&self) -> bool {
        return self.raw_bytes[INES_FLAGS_6] & 0b0000_0100 != 0;
    }

    fn _mapper_ines1(&self) -> u16 {
        let lower_nybble = (self.raw_bytes[INES_FLAGS_6] & 0b1111_0000) >> 4;
        let upper_nybble = self.raw_bytes[INES_FLAGS_7] & 0b1111_0000;
        // DiskDude! check: are the padding bytes here all zero?
        // Documented here: https://wiki.nesdev.com/w/index.php/INES#Flags_10
        if self.ines1_extended_attributes_valid() {
            // Spec compliant path
            let mapper_number = lower_nybble + upper_nybble;
            return mapper_number as u16;
        } else {
            // DiskDude! path
            // We probably have a very old ROM and a dumper's
            // signature in the padding bytes from 7-15. Since byte 7 was a
            // later addition to the spec, in this instance we should not
            // trust its contents.
            let mapper_number = lower_nybble;
            return mapper_number as u16;
        }
    }

    fn _mapper_ines2(&self) -> u16 {
        let lower_nybble = ((self.raw_bytes[INES_FLAGS_6] & 0b1111_0000) >> 4) as u16;
        let middle_nybble = (self.raw_bytes[INES_FLAGS_7] & 0b1111_0000) as u16;
        let upper_nybble = ((self.raw_bytes[INES2_MAPPER_SUB_MSB] & 0b0000_1111) as u16) << 8;
        return upper_nybble | middle_nybble | lower_nybble;
    }

    pub fn mapper_number(&self) -> u16 {
        match self.version() {
            1 => self._mapper_ines1(),
            2 => self._mapper_ines2(),
            _ => 0
        }
    }

    pub fn submapper_number(&self) -> u8 {
        match self.version() {
            1 => 0,
            2 => (self.raw_bytes[INES2_MAPPER_SUB_MSB] & 0b1111_0000) >> 4,
            _ => 0
        }
    }
}

#[derive(Clone)]
pub struct INesCartridge {
    // Internal strategy is to store each major chunk of the file as
    // raw data bytes, and then reinterpret these on the fly based
    // on the header bytes when accessed.
    pub header: INesHeader,
    pub trainer: Vec<u8>,
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub misc_rom: Vec<u8>,
}

impl INesCartridge {
    pub fn from_reader(file_reader: &mut dyn Read) -> Result<INesCartridge, INesError> {
        let mut header_bytes = [0u8; 16];
        file_reader.read_exact(&mut header_bytes)?;

        let header = INesHeader::from(&header_bytes);
        if !header.magic_header_valid() {
            return Err(INesError::InvalidHeader);
        }

        let trainer_size = if header.has_trainer() {512} else {0};
        let mut trainer: Vec<u8> = Vec::new();
        trainer.resize(trainer_size, 0);
        file_reader.read_exact(&mut trainer)?;

        let mut prg: Vec<u8> = Vec::new();
        prg.resize(header.prg_size(), 0);
        file_reader.read_exact(&mut prg)?;
        if prg.len() == 0 {
            return Err(INesError::ReadError{reason: format!("PRG ROM size is {}. This file is invalid, or at the very least quite unusual. Aborting.", prg.len())});
        }

        let mut chr: Vec<u8> = Vec::new();
        chr.resize(header.chr_rom_size(), 0);
        file_reader.read_exact(&mut chr)?;
        println!("chr rom size: {}", chr.len());

        // If there is any remaining data at this point, it becomes misc_rom and,
        // currently, has no other special handling
        let mut misc: Vec<u8> = Vec::new();
        file_reader.read_to_end(&mut misc)?;
        println!("misc_size: {}", misc.len());

        return Ok(INesCartridge {
            header: header,
            trainer: trainer,
            prg: prg,
            chr: chr,
            misc_rom: misc
        });
    }

    pub fn prg_rom_block(&self) -> MemoryBlock {
        return MemoryBlock::new(&self.prg, MemoryType::Rom);
    }

    pub fn prg_ram_blocks(&self) -> Vec<MemoryBlock> {
        let mut blocks: Vec<MemoryBlock> = Vec::new();
        if self.header.prg_ram_size() > 0 {
            let mut prg_ram: Vec<u8> = Vec::new();
            prg_ram.resize(self.header.prg_ram_size(), 0);
            blocks.push(MemoryBlock::new(&prg_ram, MemoryType::Ram));
        }
        if self.header.prg_sram_size() > 0 {
            let mut prg_sram: Vec<u8> = Vec::new();
            prg_sram.resize(self.header.prg_sram_size(), 0);
            blocks.push(MemoryBlock::new(&prg_sram, MemoryType::NvRam));
        }
        if blocks.len() == 0 {
            // Always include at least one entry in this list; in this case, a
            // single empty block.
            blocks.push(MemoryBlock::new(&Vec::new(), MemoryType::Rom));
        }
        return blocks;
    }

    pub fn chr_blocks(&self) -> Vec<MemoryBlock> {
        let mut blocks: Vec<MemoryBlock> = Vec::new();
        if self.chr.len() > 0 {
            blocks.push(MemoryBlock::new(&self.chr, MemoryType::Rom));
        }
        if self.header.chr_ram_size() > 0 {
            let mut chr_ram: Vec<u8> = Vec::new();
            chr_ram.resize(self.header.chr_ram_size(), 0);
            blocks.push(MemoryBlock::new(&chr_ram, MemoryType::Ram));
        }
        if self.header.chr_sram_size() > 0 {
            let mut chr_sram: Vec<u8> = Vec::new();
            chr_sram.resize(self.header.chr_sram_size(), 0);
            blocks.push(MemoryBlock::new(&chr_sram, MemoryType::NvRam));
        }
        if blocks.len() == 0 {
            // Always include at least one entry in this list; in this case, a
            // single empty block.
            blocks.push(MemoryBlock::new(&Vec::new(), MemoryType::Rom));
        }
        return blocks;
    }

    pub fn prg_ram_block(&self) -> Result<MemoryBlock, String> {
        let blocks = self.prg_ram_blocks();
        if blocks.len() != 1 {
            return Err(format!("Unsupported mixed PRG RAM types for mapper number {}", self.header.mapper_number()));
        }
        return Ok(blocks[0].clone());
    }

    pub fn chr_block(&self) -> Result<MemoryBlock, String> {
        let blocks = self.chr_blocks();
        if blocks.len() != 1 {
            return Err(format!("Unsupported mixed CHR types for mapper number {}", self.header.mapper_number()));
        }
        return Ok(blocks[0].clone());
    }
}
