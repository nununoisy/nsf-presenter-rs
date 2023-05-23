// This is a generic trait, which all audio channels should implement. It is
// primarily meant for use with debug features that display data about the
// audio channels in realtime.

use super::RingBuffer;

#[derive(Clone)]
pub enum PlaybackRate {
    FundamentalFrequency { frequency: f32 },
    LfsrRate { index: usize, max: usize },
    SampleRate { frequency: f32 },
}

#[derive(Clone)]
pub enum Volume {
    VolumeIndex { index: usize, max: usize },
}

#[derive(Clone)]
pub enum Timbre {
    DutyIndex { index: usize, max: usize },
    LsfrMode { index: usize, max: usize },
    PatchIndex { index: usize, max: usize },
}

pub trait AudioChannelState {
    fn name(&self) -> String;
    fn chip(&self) -> String;
    fn sample_buffer(&self) -> &RingBuffer;
    // TODO: Remove this default implementation, once edge buffer
    // is properly supported in all channel types
    fn edge_buffer(&self) -> &RingBuffer;
    fn min_sample(&self) -> i16 {return i16::MIN;}
    fn max_sample(&self) -> i16 {return i16::MAX;}
    fn record_current_output(&mut self);
    fn muted(&self) -> bool;
    fn mute(&mut self);
    fn unmute(&mut self);

    fn playing(&self) -> bool { return false; }
    fn rate(&self) -> PlaybackRate { return PlaybackRate::SampleRate{frequency: 0.0}; }
    fn volume(&self) -> Option<Volume> {return None}
    fn timbre(&self) -> Option<Timbre> {return None}
    fn amplitude(&self) -> f32 {
        /* pre-mixed volume, allows chips using non-linear mixing to tailor this value.
           results should be based on 2A03 pulse, where 1.0 corresponds to 0xF */
        if !self.playing() {return 0.0}
        match self.volume() {
            Some(Volume::VolumeIndex{index, max}) => {return index as f32 / (max + 1) as f32},
            None => {return 1.0}
        }
    }
}