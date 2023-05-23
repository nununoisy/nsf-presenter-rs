use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use rusticnes_core::apu::ApuState;
use rusticnes_core::apu::RingBuffer;
use rusticnes_core::mmc::mapper::Mapper;
use rusticnes_core::apu::AudioChannelState;

pub struct ApuWindow {
    pub canvas: SimpleBuffer,
    pub font: Font,
    pub shown: bool,
    pub waveform_height: u32,
    pub text_height: u32,
    pub spacing: u32,
    pub old_channels: usize,
}

pub fn find_edge(edge_buffer: &RingBuffer, window_size: usize) -> usize {
    let start_index = (edge_buffer.index() - window_size) % edge_buffer.buffer().len();
    let mut current_index = start_index;
    for _i in 0 .. (window_size * 4) {
        if edge_buffer.buffer()[current_index] != 0 {
            // center the window on this sample
            return (current_index - (window_size / 2)) % edge_buffer.buffer().len();
        }
        current_index = (current_index - 1) % edge_buffer.buffer().len();
    }
    // couldn't find an edge, so return the most recent slice
    return start_index;
}

impl ApuWindow {
    pub fn new() -> ApuWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return ApuWindow {
            canvas: SimpleBuffer::new(256, 1080),
            font: font,
            shown: false,
            waveform_height: 64,
            text_height: 10,
            spacing: 2,
            old_channels: 5,
        };
    }

    pub fn channel_height(&self) -> u32 {
        return self.waveform_height + self.text_height;
    }

    pub fn draw_waveform(&mut self, channel: &dyn AudioChannelState, color: Color, x: u32, y: u32, width: u32, height: u32, align: bool) {
        let audiobuffer = channel.sample_buffer().buffer();
        let mut start_index = channel.sample_buffer().index() - ((width as usize) * 2) - 1000;
        start_index = start_index % audiobuffer.len();
        if align {
            start_index = find_edge(channel.edge_buffer(), (width * 3) as usize);
        }
        
        let sample_min = channel.min_sample();
        let sample_max = channel.max_sample() + 1;
        let range = (sample_max as u32) - (sample_min as u32);
        let mut last_y = (((audiobuffer[start_index] - sample_min) as u64 * height as u64) / range as u64) as u32;
        if last_y >= height {
            last_y = height - 1;
        }
        for dx in x .. (x + width) {
            let sample_index = (start_index + (dx * 3) as usize) % audiobuffer.len();
            let sample = audiobuffer[sample_index];
            let current_x = dx as u32;
            let mut current_y = (((sample - sample_min) as u64 * height as u64) / range as u64) as u32;
            if current_y >= height {
                current_y = height - 1;
            }
            for dy in current_y .. last_y {
                self.canvas.put_pixel(current_x, y + dy, color);
            }
            for dy in last_y .. current_y {
                self.canvas.put_pixel(current_x, y + dy, color);
            }
            last_y = current_y;
            self.canvas.put_pixel(dx, y + current_y, color);
        }
    }

    pub fn channel_color(channel: &dyn AudioChannelState, index: u32) -> Color {
        if channel.muted() {
            return Color::rgb(32, 32, 32);
        }
        return match channel.chip().as_str() {
            "2A03" => match channel.name().as_str() {
                "Pulse 1" => {Color::rgb(192,  32,  32)},
                "Pulse 2" => {Color::rgb(192,  96,  32)},
                "Triangle" => {Color::rgb(32, 192,  32)},
                "Noise" => {Color::rgb(144, 144, 180)},
                "DMC" => {Color::rgb(128,  64, 192)},
                _ => {/*unreachable*/ Color::rgb(192,  192, 192)}
            },
            "MMC5" => match channel.name().as_str() {
                "Pulse 1" => {Color::rgb(224, 24, 64)},
                "Pulse 2" => {Color::rgb(180, 12, 40)},
                "PCM" => {Color::rgb(192, 12, 64)},
                _ => {/*unreachable*/ Color::rgb(192,  192, 192)}
            },
            "YM2149F" => match channel.name().as_str() {
                "A" => {Color::rgb(32, 144, 204)},
                "B" => {Color::rgb(24, 104, 228)},
                "C" => {Color::rgb(16, 64, 248)},
                _ => {/*unreachable*/ Color::rgb(192,  192, 192)}
            },
            "VRC6" => match channel.name().as_str() {
                "Pulse 1" => {Color::rgb(0x97, 0x51, 0x74)},
                "Pulse 2" => {Color::rgb(0x92, 0x49, 0x90)},
                "Sawtooth" => {Color::rgb(0x07, 0x7d, 0x5a)},
                _ => {/*unreachable*/ Color::rgb(192,  192, 192)}
            },
            "N163" => match channel.name().as_str() {
                "NAMCO 1" => {Color::rgb(0xC0, 0x20, 0x20)},
                "NAMCO 2" => {Color::rgb(0xA0, 0x10, 0x10)},
                "NAMCO 3" => {Color::rgb(0xC0, 0x20, 0x20)},
                "NAMCO 4" => {Color::rgb(0xA0, 0x10, 0x10)},
                "NAMCO 5" => {Color::rgb(0xC0, 0x20, 0x20)},
                "NAMCO 6" => {Color::rgb(0xA0, 0x10, 0x10)},
                "NAMCO 7" => {Color::rgb(0xC0, 0x20, 0x20)},
                "NAMCO 8" => {Color::rgb(0xA0, 0x10, 0x10)},
                _ => {/*unreachable*/ Color::rgb(192,  192, 192)}  
            },
            "APU" => {
                Color::rgb(192,  192, 192)
            },
            _ => {
                // Unknown mapper audio, we'll default to a drab grey
                if index % 2 != 0 {
                    Color::rgb(128, 128, 128)
                } else {
                    Color::rgb(144, 144, 144)
                }
            } 
        };
    }

    pub fn background_color(foreground_color: Color) -> Color {
        return Color::rgb(
            foreground_color.r() / 8,
            foreground_color.g() / 8,
            foreground_color.b() / 8
        );
    }

    pub fn glow_color(foreground_color: Color) -> Color {
        return Color::rgb(
            foreground_color.r() / 3,
            foreground_color.g() / 3,
            foreground_color.b() / 3
        );
    }

    pub fn draw_channel(&mut self, x: u32, y: u32, channel: &dyn AudioChannelState) {
        let index = y / self.channel_height();
        let foreground_color = ApuWindow::channel_color(channel, index);
        let background_color = ApuWindow::background_color(foreground_color);
        let glow_color = ApuWindow::glow_color(foreground_color);

        let canvas_width = self.canvas.width;
        let channel_height = self.channel_height();
        let channel_header = format!("[{}] {}", channel.chip(), channel.name());
        drawing::rect(&mut self.canvas, x, y, canvas_width, channel_height, background_color);
        drawing::text(&mut self.canvas, &self.font, x, y + 1, &channel_header, foreground_color);

        
        self.draw_waveform(channel, glow_color, 0,   y + self.text_height + 1, canvas_width,  self.waveform_height, true);
        self.draw_waveform(channel, glow_color, 0,   y + self.text_height - 1, canvas_width,  self.waveform_height, true);
        self.draw_waveform(channel, foreground_color, 0,   y + self.text_height, canvas_width,  self.waveform_height, true);
        drawing::rect(&mut self.canvas, 0, y + channel_height, canvas_width, 2, Color::rgb(12, 12, 12));
    }

    pub fn collect_channels<'a>(apu: &'a ApuState, mapper: &'a dyn Mapper) -> Vec<&'a dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        channels.extend(apu.channels());
        channels.extend(mapper.channels());
        channels.push(apu);
        return channels;
    }

    pub fn draw(&mut self, apu: &ApuState, mapper: &dyn Mapper) {
        let channels = ApuWindow::collect_channels(apu, mapper);
        if channels.len() != self.old_channels {
            self.resize_panel(apu, mapper);
            self.old_channels = channels.len();
        }

        let mut dy = self.spacing;
        for channel in channels {
            self.draw_channel(0, dy, channel);
            dy = dy + self.channel_height() + self.spacing;
        }
    }

    pub fn resize_panel(&mut self, apu: &ApuState, mapper: &dyn Mapper) {
        let channels = ApuWindow::collect_channels(apu, mapper);

        self.canvas.height = ((self.channel_height() + self.spacing) * channels.len() as u32) + self.spacing;
        let canvas_width = self.canvas.width;
        let canvas_height = self.canvas.height;
        drawing::rect(&mut self.canvas, 0, 0, canvas_width, canvas_height, Color::rgb(12, 12, 12));
    }

    pub fn mouse_mutes_channel(&mut self, apu: &ApuState, mapper: &dyn Mapper, my: i32) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        let channels = ApuWindow::collect_channels(apu, mapper);
        let channel_index = ((my as u32) / (self.channel_height() + self.spacing)) as usize;
        if channel_index < (channels.len() - 1) { // do not attempt to mute the final mix
            if channels[channel_index].muted() {
                events.push(Event::UnmuteChannel(channels[channel_index].chip(), channels[channel_index].name()))
            } else {
                events.push(Event::MuteChannel(channels[channel_index].chip(), channels[channel_index].name()))
            }
        }
        return events;
    }
}

impl Panel for ApuWindow {
    fn title(&self) -> &str {
        return "APU Surfboard";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        match event {
            Event::RequestFrame => {self.draw(&runtime.nes.apu, &*runtime.nes.mapper)},
            Event::ShowApuWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},
            Event::CartridgeLoaded(_id) => {self.resize_panel(&runtime.nes.apu, &*runtime.nes.mapper)},
            Event::MouseClick(_x, y) => {events.extend(self.mouse_mutes_channel(&runtime.nes.apu, &*runtime.nes.mapper, y));},
            _ => {}
        }
        return events;
    }
    
    fn active_canvas(&self) -> &SimpleBuffer {
        return &self.canvas;
    }

    fn scale_factor(&self) -> u32 {
        return 2;
    }
}