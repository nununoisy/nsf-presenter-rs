use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use std::time::Instant;

use rusticnes_core::nes::NesState;
use rusticnes_core::palettes::NTSC_PAL;

pub struct GameWindow {
    pub canvas: SimpleBuffer,
    pub font: Font,
    pub shown: bool,
    pub scale: u32,
    pub simulate_overscan: bool,
    pub ntsc_filter: bool,
    pub display_fps: bool,

    pub frame_duration: Instant,
    pub durations: [f32; 60],
    pub duration_index: usize,
    pub measured_fps: f32,
}

impl GameWindow {
    pub fn new() -> GameWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return GameWindow {
            canvas: SimpleBuffer::new(240, 224),
            font: font,
            shown: true,
            scale: 2,
            simulate_overscan: false,
            ntsc_filter: false,
            display_fps: false,

            frame_duration: Instant::now(),
            durations: [0f32; 60],
            duration_index: 0,
            measured_fps: 0.0,
        };
    }

    fn update_fps(&mut self) {
        let time_since_last = self.frame_duration.elapsed().as_millis() as f32;
        self.frame_duration = Instant::now();
        self.durations[self.duration_index] = time_since_last;
        self.duration_index = (self.duration_index + 1) % 60;
        let average_frame_duration_millis = self.durations.iter().sum::<f32>() as f32 / (self.durations.len() as f32);
        if average_frame_duration_millis > 0.0 {
            self.measured_fps = 1000.0 / average_frame_duration_millis;
        }
    }

    fn draw(&mut self, nes: &NesState) {
        let overscan: u32 = if self.simulate_overscan {8} else {0};

        // Update the game screen
        for x in overscan .. 256 - overscan {
            for y in overscan .. 240 - overscan {
                if self.ntsc_filter {
                    let scale = self.scale;
                    let base_x = x * scale;
                    let base_y = y * 256 * scale;

                    for sx in 0 .. self.scale {
                        let column_color = Color::from_raw(nes.ppu.filtered_screen[(base_y + base_x + sx) as usize]);
                        for sy in 0 .. self.scale {
                            self.canvas.put_pixel((x - overscan) * scale + sx, (y - overscan) * scale + sy, column_color);        
                        }
                    }
                } else {
                    let palette_index = ((nes.ppu.screen[(y * 256 + x) as usize]) as usize) * 3;
                    self.canvas.put_pixel(
                        x - overscan,
                        y - overscan,
                        Color::rgb(
                            NTSC_PAL[palette_index + 0],
                            NTSC_PAL[palette_index + 1],
                            NTSC_PAL[palette_index + 2])
                    );
                }
            }
        }

        if self.display_fps {
            let fps_display = format!("FPS: {:.2}", self.measured_fps);
            drawing::text(&mut self.canvas, &self.font, 5, 5, &fps_display, Color::rgba(255, 255, 255, 192));
        }
    }

    fn increase_scale(&mut self) {
        if self.scale < 8 {
            self.scale += 1;
        }
        self.update_canvas_size();
    }

    fn decrease_scale(&mut self) {
        if self.scale > 1 {
            self.scale -= 1;
        }
        self.update_canvas_size();
    }

    fn update_canvas_size(&mut self) {
        let base_width = if self.simulate_overscan {240} else {256};
        let base_height = if self.simulate_overscan {224} else {240};
        let scaled_width = if self.ntsc_filter {base_width * self.scale} else {base_width};
        let scaled_height = if self.ntsc_filter {base_height * self.scale} else {base_height};
        self.canvas = SimpleBuffer::new(scaled_width, scaled_height);
    }
}

impl Panel for GameWindow {
    fn title(&self) -> &str {
        return "RusticNES";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        let mut responses = Vec::<Event>::new();
        match event {
            Event::RequestFrame => {
                self.update_fps();
                self.draw(&runtime.nes);
                // Technically this will have us drawing one frame behind the filter. To fix
                // this, we'd need Application to manage filters instead.
                if self.ntsc_filter {
                    responses.push(Event::NesRenderNTSC(256 * (self.scale as usize)));
                }
            },
            Event::ShowGameWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},

            Event::GameIncreaseScale => {
                self.increase_scale();
                responses.push(Event::StoreIntegerSetting("video.scale_factor".to_string(), self.scale as i64));
            },
            Event::GameDecreaseScale => {
                self.decrease_scale();
                responses.push(Event::StoreIntegerSetting("video.scale_factor".to_string(), self.scale as i64));
            },

            Event::ApplyBooleanSetting(path, value) => {
                match path.as_str() {
                    "video.display_fps" => {self.display_fps = value},
                    "video.ntsc_filter" => {self.ntsc_filter = value; self.update_canvas_size()},
                    "video.simulate_overscan" => {self.simulate_overscan = value; self.update_canvas_size()},
                    _ => {}
                }
            },
            Event::ApplyIntegerSetting(path, value) => {
                match path.as_str() {
                    "video.scale_factor" => {
                        if value > 0 && value < 8 {
                            self.scale = value as u32;
                            self.update_canvas_size();
                        }
                    },
                    _ => {}
                }
            },
            _ => {}
        }
        return responses;
    }
    
    fn active_canvas(&self) -> &SimpleBuffer {
        return &self.canvas;
    }

    fn scale_factor(&self) -> u32 {
        if self.ntsc_filter {
            return 1; // we handle scale in software
        } else {
            return self.scale;
        }
    }
}