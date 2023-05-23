// NSF, a container format for NES music data and the 6502 code
// necessary to play it back. Often used to house ripped music engines
// from commercial games, but sees additional popularity for modern
// chiptune artists and occasionally indie games.
// https://wiki.nesdev.com/w/index.php/NSF

// RusticNES is an NES emulator, so it will be attempting to mimick
// the limitations of a hardware player. Some advanced NSF files,
// especially those with many expansion chips or a fast playback rate,
// may not perform correctly in RusticNES, just as they would fail in most
// hardware NSF player implementations. This is a feature, not a bug.

use std::io::Read;
use std::error::Error;
use std::fmt;

#[derive(Copy, Clone)]
pub struct NsfHeader {
    raw_bytes: [u8; 0x80]
}

const MSDOS_EOF: u8 = 0x1A;

const NSF_MAGIC_N: usize = 0x000;
const NSF_MAGIC_E: usize = 0x001;
const NSF_MAGIC_S: usize = 0x002;
const NSF_MAGIC_M: usize = 0x003;
const NSF_MAGIC_EOF: usize = 0x004;
const NSF_VERSION: usize = 0x005;
const NSF_TOTAL_SONGS: usize = 0x006;
const NSF_STARTING_SONG: usize = 0x007;
const NSF_LOAD_ADDR: usize = 0x008;
const NSF_INIT_ADDR: usize = 0x00A;
const NSF_PLAY_ADDR: usize = 0x00C;
const NSF_SONG_NAME: usize = 0x00E;
const NSF_ARTIST_NAME: usize = 0x02E;
const NSF_COPYRIGHT_HOLDER: usize = 0x04E;
const NSF_NTSC_PLAY_SPEED: usize = 0x06E;
const NSF_BANK_INIT: usize = 0x070;
const NSF_PAL_PLAY_SPEED: usize = 0x078;
//const NSF_NTSC_PAL_SELECTION: usize = 0x07A;
const NSF_EXPANSION_CHIPS: usize = 0x07B;
//const NSF2_FLAGS: usize = 0x07C;
const NSF_PRG_LENGTH: usize = 0x07D;

impl NsfHeader {
    pub fn from(raw_bytes: &[u8]) -> NsfHeader {
        let mut header = NsfHeader {
            raw_bytes: [0u8; 0x80],
        };
        header.raw_bytes.copy_from_slice(&raw_bytes[0..0x80]);
        return header;
    }

    pub fn magic_header_valid(&self) -> bool {
        return 
            self.raw_bytes[NSF_MAGIC_N] as char == 'N' &&
            self.raw_bytes[NSF_MAGIC_E] as char == 'E' &&
            self.raw_bytes[NSF_MAGIC_S] as char == 'S' &&
            self.raw_bytes[NSF_MAGIC_M] as char == 'M' &&
            self.raw_bytes[NSF_MAGIC_EOF] == MSDOS_EOF;
    }

    pub fn version_number(&self) -> u8 {
        return self.raw_bytes[NSF_VERSION];
    }

    pub fn total_songs(&self) -> u8 {
        return self.raw_bytes[NSF_TOTAL_SONGS];
    }

    pub fn starting_song(&self) -> u8 {
        return self.raw_bytes[NSF_STARTING_SONG];
    }

    pub fn _word(&self, offset: usize) -> u16 {
        let addr_low =   self.raw_bytes[offset + 0] as u16;
        let addr_high = (self.raw_bytes[offset + 1] as u16) << 8;
        return addr_low + addr_high;
    }

    pub fn load_address(&self) -> u16 {
        return self._word(NSF_LOAD_ADDR);
    }

    pub fn init_address(&self) -> u16 {
        return self._word(NSF_INIT_ADDR);
    }

    pub fn play_address(&self) -> u16 {
        return self._word(NSF_PLAY_ADDR);
    }

    /* strings are complicated, let's skip them for now */

    pub fn ntsc_playback_speed(&self) -> u16 {
        return self._word(NSF_NTSC_PLAY_SPEED);
    }

    pub fn pal_playback_speed(&self) -> u16 {
        return self._word(NSF_PAL_PLAY_SPEED);
    }

    pub fn initial_banks(&self) -> Vec<usize> {
        return vec![
            self.raw_bytes[NSF_BANK_INIT + 0] as usize,
            self.raw_bytes[NSF_BANK_INIT + 1] as usize,
            self.raw_bytes[NSF_BANK_INIT + 2] as usize,
            self.raw_bytes[NSF_BANK_INIT + 3] as usize,
            self.raw_bytes[NSF_BANK_INIT + 4] as usize,
            self.raw_bytes[NSF_BANK_INIT + 5] as usize,
            self.raw_bytes[NSF_BANK_INIT + 6] as usize,
            self.raw_bytes[NSF_BANK_INIT + 7] as usize,
        ];
    }

    pub fn is_bank_switched(&self) -> bool {
        return 
            (self.raw_bytes[NSF_BANK_INIT + 0] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 1] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 2] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 3] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 4] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 5] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 6] != 0) ||
            (self.raw_bytes[NSF_BANK_INIT + 7] != 0);
    }

    pub fn program_length(&self) -> usize {
        let addr_low =   self.raw_bytes[NSF_PRG_LENGTH + 0] as usize;
        let addr_mid =  (self.raw_bytes[NSF_PRG_LENGTH + 1] as usize) << 8;
        let addr_high = (self.raw_bytes[NSF_PRG_LENGTH + 2] as usize) << 16;
        return addr_low + addr_mid + addr_high;
    }

    pub fn vrc6(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0000_0001) != 0;
    }

    pub fn vrc7(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0000_0010) != 0;
    }

    pub fn fds(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0000_0100) != 0;
    }

    pub fn mmc5(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0000_1000) != 0;
    }

    pub fn n163(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0001_0000) != 0;
    }

    pub fn s5b(&self) -> bool {
        return (self.raw_bytes[NSF_EXPANSION_CHIPS] & 0b0010_0000) != 0;
    }

    pub fn song_name(&self) -> Vec<u8> {
        return self.raw_bytes[NSF_SONG_NAME ..= (NSF_SONG_NAME + 32)].to_vec();
    }

    pub fn artist_name(&self) -> Vec<u8> {
        return self.raw_bytes[NSF_ARTIST_NAME ..= (NSF_ARTIST_NAME + 32)].to_vec();
    }

    pub fn copyright_holder(&self) -> Vec<u8> {
        return self.raw_bytes[NSF_COPYRIGHT_HOLDER ..= (NSF_COPYRIGHT_HOLDER + 32)].to_vec();
    }
}

#[derive(Debug)]
pub enum NsfError {
    InvalidHeader,
    Unimplemented,
    ReadError{reason: String}
}

impl Error for NsfError {}

impl fmt::Display for NsfError  {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NsfError::InvalidHeader => {write!(f, "Invalid NSF Header")},
            NsfError::Unimplemented => {write!(f, "Unimplemented (Lazy programmers!!1)")},
            NsfError::ReadError{reason} => {write!(f, "Error reading cartridge: {}", reason)}
        }
    }
}

impl From<std::io::Error> for NsfError {
    fn from(error: std::io::Error) -> Self {
        return NsfError::ReadError{reason: error.to_string()};
    }
}

#[derive(Clone)]
pub struct NsfFile {
    // Internal strategy is to store each major chunk of the file as
    // raw data bytes, and then reinterpret these on the fly based
    // on the header bytes when accessed.
    pub header: NsfHeader,
    pub prg: Vec<u8>,
    pub metadata: Vec<u8>,
}

impl NsfFile {
    pub fn from_reader(file_reader: &mut dyn Read) -> Result<NsfFile, NsfError> {
        let mut header_bytes = [0u8; 0x80];
        file_reader.read_exact(&mut header_bytes)?;

        let header = NsfHeader::from(&header_bytes);
        if !header.magic_header_valid() {
            return Err(NsfError::InvalidHeader);
        }

        let mut prg: Vec<u8> = Vec::new();
        let mut metadata: Vec<u8> = Vec::new();
        if header.program_length() == 0 {
            // There is no explicit length, so consider the entire rest of the file
            // to be program data
            file_reader.read_to_end(&mut prg)?;
        } else {
            // The size specifies only the program data area
            prg.resize(header.program_length(), 0);
            file_reader.read_exact(&mut prg)?;
            // Everything else is "metadata" which for NSF2 might be parsed out separately.
            // It should not be considered part of the rom image.
            file_reader.read_to_end(&mut metadata)?;
        }

        if header.is_bank_switched() {
            // Pad the beginning of this data with zero bytes up to the load address
            let padding_bytes = (header.load_address() & 0x0FFF) as usize;
            let mut rom_image = Vec::new();
            rom_image.resize(padding_bytes, 0);
            rom_image.extend(prg);
            // If the final length at this point is not a multiple of 4k, the size of one PRG bank,
            // then we now additionally extend it to fill out the last bank to this boundary
            if rom_image.len() % 0x1000 != 0 {
                let alignment_shortage = 0x1000 - (rom_image.len() % 0x1000);
                let aligned_size = rom_image.len() + alignment_shortage;
                rom_image.resize(aligned_size, 0);
            }
            prg = rom_image;
        }

        return Ok(NsfFile {
            header: header,
            prg: prg,
            metadata: metadata
        });
    }
}

