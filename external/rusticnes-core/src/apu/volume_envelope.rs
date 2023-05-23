pub struct VolumeEnvelopeState {
    // Volume Envelope
    pub volume_register: u8,
    pub decay: u8,
    pub divider: u8,
    pub enabled: bool,
    pub looping: bool,
    pub start_flag: bool,
}

impl VolumeEnvelopeState {
    pub fn new() -> VolumeEnvelopeState {
        return VolumeEnvelopeState {
            volume_register: 0,
            decay: 0,
            divider: 0,
            enabled: false,
            looping: false,
            start_flag: false,
        }
    }

    pub fn current_volume(&self) -> u8 {
        if self.enabled {
            return self.decay;
        } else {
            return self.volume_register;
        }
    }

    pub fn clock(&mut self) {
        if self.start_flag {
            self.decay = 15;
            self.start_flag = false;
            self.divider = self.volume_register;
        } else {
            // Clock the divider
            if self.divider == 0 {
                self.divider = self.volume_register;
                if self.decay > 0 {
                    self.decay -= 1;
                } else {
                    if self.looping {
                        self.decay = 15;
                    }
                }
            } else {
                self.divider = self.divider - 1;
            }
        }
    }
}