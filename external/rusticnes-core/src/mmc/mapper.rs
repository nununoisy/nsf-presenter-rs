use apu::AudioChannelState;

#[derive(Copy, Clone, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    OneScreenLower,
    OneScreenUpper,
    FourScreen,
}

pub fn mirroring_mode_name(mode: Mirroring) -> &'static str {
    match mode {
        Mirroring::Horizontal => "Horizontal",
        Mirroring::Vertical => "Vertical",
        Mirroring::OneScreenLower => "OneScreenLower",
        Mirroring::OneScreenUpper => "OneScreenUpper",
        Mirroring::FourScreen => "FourScreen"
    }
}

pub trait Mapper: Send {
    fn read_cpu(&mut self, address: u16) -> Option<u8> {return self.debug_read_cpu(address);}
    fn write_cpu(&mut self, address: u16, data: u8);
    fn access_ppu(&mut self, _address: u16) {}
    fn read_ppu(&mut self, address: u16) -> Option<u8> {return self.debug_read_ppu(address);}
    fn write_ppu(&mut self, address: u16, data: u8);
    fn debug_read_cpu(&self, address: u16) -> Option<u8>;
    fn debug_read_ppu(&self, address: u16) -> Option<u8>;
    fn print_debug_status(&self) {}
    fn mirroring(&self) -> Mirroring;
    fn has_sram(&self) -> bool {return false;}
    fn get_sram(&self) -> Vec<u8> {return vec![0u8; 0];}
    fn load_sram(&mut self, _: Vec<u8>) {}
    fn irq_flag(&self) -> bool {return false;}
    fn clock_cpu(&mut self) {}
    fn mix_expansion_audio(&self, nes_sample: f32) -> f32 {return nes_sample;}
    fn channels(&self) ->  Vec<& dyn AudioChannelState> {return Vec::new();}
    fn channels_mut(&mut self) ->  Vec<&mut dyn AudioChannelState> {return Vec::new();}
    fn record_expansion_audio_output(&mut self, _nes_sample: f32) {}
    fn nsf_set_track(&mut self, _track_index: u8) {}
    fn nsf_manual_mode(&mut self) {}
    fn audio_multiplexing(&mut self, _emulate: bool) {}
}
