// A dummy mapper with no loaded data. Useful for initializing an NesState
// with no actual cartridge loaded.

use mmc::mapper::*;

pub struct NoneMapper {
}

impl NoneMapper {
    pub fn new() -> NoneMapper {
        return NoneMapper {
        }
    }
}

impl Mapper for NoneMapper {
    fn mirroring(&self) -> Mirroring {
        return Mirroring::Horizontal;
    }
    
    fn debug_read_cpu(&self, _: u16) -> Option<u8> {
        return None;
    }

    fn debug_read_ppu(&self, _: u16) -> Option<u8> {
        return None;
    }

    fn write_cpu(&mut self, _: u16, _: u8) {
        //Do nothing
    }

    fn write_ppu(&mut self, _: u16, _: u8) {
        //Do nothing
    }    
}
