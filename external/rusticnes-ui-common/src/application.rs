use std::rc::Rc;

use events::Event;
use events::StandardControllerButton;

use settings::SettingsState;

use rusticnes_core::nes::NesState;
use rusticnes_core::mmc::none::NoneMapper;
use rusticnes_core::cartridge::mapper_from_file;

use rusticnes_core::apu::AudioChannelState;



pub struct RuntimeState {
    pub nes: NesState,
    pub running: bool,
    pub file_loaded: bool,
    pub last_frame: u32,
    pub last_scanline: u16,
    pub last_apu_quarter_frame_count: u32,
    pub last_apu_half_frame_count: u32,
    pub settings: SettingsState,
}

impl RuntimeState {
    pub fn new() -> RuntimeState {
        return RuntimeState {
            nes: NesState::new(Box::new(NoneMapper::new())),
            file_loaded: false,
            running: false,
            last_frame: 0,
            last_scanline: 0,
            last_apu_quarter_frame_count: 0,
            last_apu_half_frame_count: 0,
            settings: SettingsState::new(),
        }
    }

    pub fn load_cartridge(&mut self, cart_id: String, file_data: &[u8]) -> Event {
        let maybe_mapper = mapper_from_file(file_data);
        match maybe_mapper {
            Ok(mapper) => {
                self.nes = NesState::new(mapper);
                self.nes.power_on();
                self.running = true;
                self.file_loaded = true;
                return Event::CartridgeLoaded(cart_id);
            },
            Err(why) => {
                return Event::CartridgeRejected(cart_id, why);
            }
        }
    }

    pub fn load_sram(&mut self, file_data: &[u8]) {
        if self.nes.mapper.has_sram() {
            if file_data.len() > 0 {
                self.nes.set_sram(file_data.to_vec());
            }
        }
    }

    pub fn button_press(&mut self, player_index: usize, button: StandardControllerButton) {
        let controllers = [
            &mut self.nes.p1_input,
            &mut self.nes.p2_input
        ];

        if player_index > controllers.len() {
            return;
        }

        let old_controller_byte = *controllers[player_index];
        let pressed_button = 0b1 << (button.clone() as u8);
        let new_controller_byte = old_controller_byte | pressed_button;
        let fixed_controller_byte = fix_dpad(new_controller_byte, button.clone());
        *controllers[player_index] = fixed_controller_byte;
    }

    pub fn button_release(&mut self, player_index: usize, button: StandardControllerButton) {
        let controllers = [
            &mut self.nes.p1_input,
            &mut self.nes.p2_input
        ];

        if player_index > controllers.len() {
            return;
        }

        let old_controller_byte = *controllers[player_index];
        let released_button = 0b1 << (button as u8);
        let release_mask = 0b1111_1111 ^ released_button;
        let new_controller_byte = old_controller_byte & release_mask;
        *controllers[player_index] = new_controller_byte;
    }

    pub fn collect_timing_events(&mut self) -> Vec<Event> {
        let mut responses: Vec<Event> = Vec::new();
        if self.nes.ppu.current_frame != self.last_frame {
            responses.push(Event::NesNewFrame);
            self.last_frame = self.nes.ppu.current_frame;
        }
        if self.nes.ppu.current_scanline != self.last_scanline {
            responses.push(Event::NesNewScanline);
            self.last_scanline = self.nes.ppu.current_scanline;
        }
        if self.nes.apu.quarter_frame_counter != self.last_apu_quarter_frame_count {
            responses.push(Event::NesNewApuQuarterFrame);
            self.last_apu_quarter_frame_count = self.nes.apu.quarter_frame_counter
        }
        if self.nes.apu.half_frame_counter != self.last_apu_half_frame_count {
            responses.push(Event::NesNewApuHalfFrame);
            self.last_apu_half_frame_count = self.nes.apu.half_frame_counter
        }
        return responses;
    }

    pub fn handle_event(&mut self, event: Event) -> Vec<Event> {
        let mut responses: Vec<Event> = Vec::new();
        responses.extend(self.settings.handle_event(event.clone()));
        match event {
            Event::ApplyBooleanSetting(path, value) => {
                match path.as_str() {
                    "audio.multiplexing" => {self.nes.mapper.audio_multiplexing(value)},
                    _ => {}
                }
            },
            Event::MuteChannel(chip_name, channel_name) => {
                let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
                channels.extend(self.nes.apu.channels_mut());
                channels.extend(self.nes.mapper.channels_mut());
                for channel in channels {
                    if channel.chip() == chip_name && channel.name() == channel_name {
                        channel.mute();
                    }
                }
                //self.nes.apu.mute_channel(&mut *self.nes.mapper, channel_index);
            },
            Event::UnmuteChannel(chip_name, channel_name) => {
                let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
                channels.extend(self.nes.apu.channels_mut());
                channels.extend(self.nes.mapper.channels_mut());
                for channel in channels {
                    if channel.chip() == chip_name && channel.name() == channel_name {
                        channel.unmute();
                    }
                }
                //self.nes.apu.unmute_channel(&mut *self.nes.mapper, channel_index);  
            },
            
            Event::LoadCartridge(cart_id, file_data, sram_data) => {
                responses.push(self.load_cartridge(cart_id, &file_data));
                self.load_sram(&sram_data);
                // Loading a new cartridge replaces the mapper and resets NesState, so we should
                // reload all settings to make sure any emulation-specific things get re-appled.
                responses.extend(self.settings.apply_settings());
            },
            Event::LoadSram(sram_data) => {
                self.load_sram(&sram_data);
            },
            Event::NesRunCycle => {
                self.nes.cycle();
                responses.extend(self.collect_timing_events());
            },
            Event::NesRunFrame => {
                self.nes.run_until_vblank();
                responses.extend(self.collect_timing_events());
            },
            Event::NesRenderNTSC(width) => {
                self.nes.ppu.render_ntsc(width);
            },
            Event::NesRunOpcode => {
                self.nes.step();
            },
            Event::NesRunScanline => {
                self.nes.run_until_hblank();
                responses.extend(self.collect_timing_events());
            },
            Event::NesReset => {
                self.nes.reset();
            },
            
            // These three events should ideally move to some sort of FrameTiming manager
            Event::NesPauseEmulation => {
                self.running = false;
            },
            Event::NesResumeEmulation => {
                self.running = true;
            },
            Event::NesToggleEmulation => {
                self.running = !self.running;
            },

            Event::NesNudgeAlignment => {
                self.nes.nudge_ppu_alignment();
            }

            Event::RequestSramSave(sram_id) => {
                if self.nes.mapper.has_sram()  {
                    responses.push(Event::SaveSram(sram_id, Rc::new(self.nes.sram())));
                }
            },

            // Input is due for an overhaul. Ideally the IoBus should handle its own
            // events, rather than doing this here.
            Event::StandardControllerPress(controller_index, button) => {
                self.button_press(controller_index, button);
            },
            Event::StandardControllerRelease(controller_index, button) => {
                self.button_release(controller_index, button);
            },
            _ => {}
        }
        return responses;
    }
}

pub fn fix_dpad(controller_byte: u8, last_button_pressed: StandardControllerButton) -> u8 {
    let mut fixed_byte = controller_byte;
    match last_button_pressed {
        StandardControllerButton::DPadUp => {fixed_byte &= 0b1101_1111},
        StandardControllerButton::DPadDown => {fixed_byte &= 0b1110_1111},
        StandardControllerButton::DPadLeft => {fixed_byte &= 0b0111_1111},
        StandardControllerButton::DPadRight => {fixed_byte &= 0b1011_1111},
        _ => {}
    }

    return fixed_byte;
}