use std::str;
use crate::emulator::nsfeparser::NsfeMetadata;

pub fn find_subsequence<T>(haystack: &[T], needle: &[T]) -> Option<usize>
    where for<'a> &'a [T]: PartialEq
{
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[derive(Copy, Clone, PartialEq)]
pub enum NsfDriverType {
    Unknown,
    FTClassic,
    FT0CC,
    FTDn
}

#[derive(Clone)]
pub struct Nsf {
    raw_bytes: Vec<u8>,
    memoized_driver_type: NsfDriverType
}

fn determine_driver_type(raw_bytes: &[u8]) -> NsfDriverType {
    if find_subsequence(&raw_bytes, b"FTDRV").is_some() {
        NsfDriverType::FTClassic
    } else if find_subsequence(&raw_bytes, b"0CCFT").is_some() {
        NsfDriverType::FT0CC
    } else if find_subsequence(&raw_bytes, b"DN-FT").is_some()
        || find_subsequence(&raw_bytes, b"Dn-FT").is_some() {
        NsfDriverType::FTDn
    } else {
        NsfDriverType::Unknown
    }
}


macro_rules! string_fn {
    ($name: tt, $offset: literal, $max_len: literal) => {
        pub fn $name(&self) -> Result<String, String> {
            self.parse_string($offset, $max_len)
        }
    }
}

macro_rules! bitflag_fn {
    ($offset: literal, $name: tt, $mask: literal) => {
        pub fn $name(&self) -> bool {
            (self.raw_bytes[$offset] & $mask) != 0
        }
    }
}

macro_rules! expansion_chip_fn {
    ($name: tt, $mask: literal) => {
        bitflag_fn!(0x7B, $name, $mask);
    }
}

macro_rules! nsf2_feature_fn {
    ($name: tt, $mask: literal) => {
        bitflag_fn!(0x7C, $name, $mask);
    }
}

impl Nsf {
    pub fn from(raw_bytes: &[u8]) -> Nsf {
        Nsf {
            raw_bytes: raw_bytes.to_vec(),
            memoized_driver_type: determine_driver_type(&raw_bytes)
        }
    }

    pub fn magic_valid(&self) -> bool {
        &self.raw_bytes[..5] == b"NESM\x1A"
    }

    pub fn version(&self) -> u8 {
        self.raw_bytes[5]
    }

    pub fn songs(&self) -> u8 {
        self.raw_bytes[6]
    }

    pub fn starting_song(&self) -> u8 {
        self.raw_bytes[7]
    }

    fn parse_string(&self, offset: usize, max_len: usize) -> Result<String, String> {
        let end = (offset..offset+max_len)
            .position(|i| self.raw_bytes[i] == 0)
            .unwrap_or(max_len);

        match str::from_utf8(&self.raw_bytes[offset..offset+end]) {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(e.to_string())
        }
    }

    string_fn!(title, 0xE, 0x20);
    string_fn!(artist, 0x2E, 0x20);
    string_fn!(copyright, 0x4E, 0x20);

    expansion_chip_fn!(vrc6, 0b0000_0001);
    expansion_chip_fn!(vrc7, 0b0000_0010);
    expansion_chip_fn!(fds, 0b0000_0100);
    expansion_chip_fn!(mmc5, 0b0000_1000);
    expansion_chip_fn!(n163, 0b0001_0000);
    expansion_chip_fn!(s5b, 0b0010_0000);

    pub fn driver_type(&self) -> NsfDriverType {
        if self.magic_valid() {
            self.memoized_driver_type
        } else {
            NsfDriverType::Unknown
        }
    }

    nsf2_feature_fn!(nsf2_irq, 0b0001_0000);
    nsf2_feature_fn!(nsf2_nonreturning_init, 0b0010_0000);
    nsf2_feature_fn!(nsf2_no_play_subroutine, 0b0100_0000);
    nsf2_feature_fn!(nsf2_has_metadata, 0b1000_0000);

    fn nsf2_program_length(&self) -> u32 {
        (u32::from_le_bytes((&self.raw_bytes[0x7C..0x80]).try_into().unwrap()) & 0xFFFFFF00) >> 8
    }

    pub fn nsfe_metadata(&self) -> Option<NsfeMetadata> {
        let metadata_offset = match (self.version(), self.nsf2_has_metadata()) {
            (2, true) => self.nsf2_program_length() as usize + 0x80,
            _ => return None
        };

        match NsfeMetadata::from(&self.raw_bytes[metadata_offset..]) {
            Ok(d) => Some(d),
            Err(e) => {
                println!("NSFe metadata parse error: {}", e);
                None
            }
        }
    }
}
