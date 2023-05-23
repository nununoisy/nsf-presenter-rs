use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use rusticnes_core::nes::NesState;
use rusticnes_core::palettes::NTSC_PAL;
use rusticnes_core::tracked_events::TrackedEvent;
use rusticnes_core::tracked_events::EventType;

pub struct EventWindow {
    pub canvas: SimpleBuffer,
    pub font: Font,
    pub shown: bool,
    pub scale: u32,
    pub mx: i32,
    pub my: i32,
}

fn cpu_register_label(address: u16) -> String {
    let label = match address {
        0x2000 => "PPUCTRL",
        0x2001 => "PPUMASK",
        0x2002 => "PPUSTATUS",
        0x2003 => "OAMADDR",
        0x2004 => "OAMDATA",
        0x2005 => "PPUSCROLL",
        0x2006 => "PPUADDR",
        0x2007 => "PPUDATA",

        0x4014 => "OAMDMA",
        _ => {""}
    };

    return label.to_string();
}

fn cpu_register_color(address: u16) -> Color {
    match address {
        0x2000 => Color::rgb(255, 94, 94),
        0x2001 => Color::rgb(142, 51, 255),
        0x2002 => Color::rgb(26, 86, 100),
        0x2003 => Color::rgb(255, 132, 224),
        0x2004 => Color::rgb(250, 255, 57),
        0x2005 => Color::rgb(46, 255, 40),
        0x2006 => Color::rgb(61, 45, 255),
        0x2007 => Color::rgb(255, 6, 13),

        0x4000 ..= 0x4013  => Color::rgb(255, 192, 0),

        0x4014 => Color::rgb(255, 0, 255),
        0x4015 => Color::rgb(255, 0, 255),
        0x4017 => Color::rgb(255, 0, 255),
        _ => Color::rgb(192, 192, 192)
    }
}

fn longest(strings: &Vec<String>)  -> usize {
    let mut length = 0;
    for string in strings {
        if string.len() > length {
            length = string.len();
        }
    }
    return length;
}

fn freshness(scanline: u16, cycle: u16, current_scanline: u16, current_cycle: u16) -> f32 {
    let test_progress = (scanline as u32) * 341 + (cycle as u32);
    let current_progress = (current_scanline as u32) * 341 + (current_cycle as u32);
    let max_distance = 262 * 341;
    let distance = (test_progress + max_distance - current_progress) % max_distance;
    return (distance as f32) / (max_distance as f32);
}

impl EventWindow {
    pub fn new() -> EventWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return EventWindow {
            canvas: SimpleBuffer::new(341, 262),
            font: font,
            shown: false,
            scale: 2,
            mx: 0,
            my: 0,
        };
    }

    fn tooltip_visible(&mut self, event: TrackedEvent) -> bool {
        // Only draw the tooltip if our mouse coordinates are favorable!
        let x = event.cycle as u32;
        let y = event.scanline as u32;

        return x == (self.mx as u32) && y == (self.my as u32)
    }

    fn draw_tooltip(&mut self, event: TrackedEvent) {
        let outline_color = Color::rgb(0x80, 0x80, 0x40);
        let background_color = Color::rgb(0xFF, 0xFF, 0xE0);
        let font_color = Color::rgb(0x20, 0x20, 0x05);
        let shadow_color = Color::rgb(0xE0, 0xE0, 0xD0);

        let title = match event.event_type {
            EventType::CpuRead{address, data: _, program_counter: _} => {
                let label = cpu_register_label(address);
                format!("Read: {}", label)
            },
            EventType::CpuWrite{address, data: _, program_counter: _} => {
                let label = cpu_register_label(address);
                format!("Write: {}", label)
            },
            EventType::CpuExecute{program_counter, data: _} => {
                let label = cpu_register_label(program_counter);
                format!("Execute: {}", label)
            },
            _ => {format!("Huh!?")}
        };

        // variable things for each event, based on its type
        let mut contents = match event.event_type {
            EventType::CpuRead{program_counter, address, data} => {
                vec![
                    format!("PC:       ${:04X}", program_counter),
                    format!("Address:  ${:04X}", address),
                    format!("Data:     ${:02X} ({})", data, data),
                ]
            },
            EventType::CpuWrite{program_counter, address, data} => {
                vec![
                    format!("PC:       ${:04X}", program_counter),
                    format!("Address:  ${:04X}", address),
                    format!("Data:     ${:02X} ({})", data, data)
                ]
            },
            EventType::CpuExecute{program_counter, data} => {
                vec![
                    format!("PC:       ${:04X}", program_counter),
                    format!("Data:     ${:02X} ({})", data, data)
                ]
            },
            _ => {vec![format!("I don't recognize this junk!")]}
        };

        // constant timing data we will always display
        contents.insert(0, format!("Scanline: {}", event.scanline));
        contents.insert(1, format!("Cycle:    {}", event.cycle));


        let mut x = (event.cycle + 5) as u32;
        let mut y = (event.scanline + 5) as u32;
        let padding = 5;
        let line_spacing = 4;
        let widest_string = std::cmp::max(longest(&contents), title.len()) as u32;
        let width = widest_string * 8 + (padding * 2) + 2;
        let text_lines = (2 + contents.len()) as u32;
        let height = text_lines * 8 + (text_lines - 1) * line_spacing + (padding * 2) + 2;

        // if we will exceed the bottom or right edge of the screen, adjust our position accordingly!
        if x + width > 339 {
            x -= width + 10;
        }
        if y + height > 260 {
            y -= height + 10;
        }

        // Draw a box!
        drawing::blend_rect(&mut self.canvas, x + 1, y + 1, width, height, Color::rgba(0, 0, 0, 0x16));
        drawing::rect(&mut self.canvas, x, y, width, height, outline_color);
        drawing::rect(&mut self.canvas, x + 1, y + 1, width - 2, height - 2, background_color);

        // Draw a title!
        drawing::text(&mut self.canvas, &self.font, x + 1 + padding + 1, y + 1 + padding + 1, &title, shadow_color);
        drawing::text(&mut self.canvas, &self.font, x + 1 + padding, y + 1 + padding, &title, font_color);

        // Draw the box contents
        let cx = x + padding + 1;
        let mut cy = y + padding + 1 + (8 * 2) + (line_spacing * 2);
        for line in contents {
            drawing::text(&mut self.canvas, &self.font, cx + 1, cy + 1, &line, shadow_color);
            drawing::text(&mut self.canvas, &self.font, cx, cy, &line, font_color);
            cy += 8 + line_spacing;
        }
    }

    fn draw_event_dot(&mut self, event: TrackedEvent, color: Color) {
        // the event outline, a bit darker
        let outline_color = Color::rgb(color.r() / 2, color.g() / 2, color.b() / 2);

        let x = event.cycle as u32;
        let y = event.scanline as u32;
        // Make the outline be very fancy and gracefully handle canvas edges
        // (todo later: make this part of rect?)
        let mut rx = x;
        let mut ry = y;
        let mut rw = 1;
        let mut rh = 1;
        if x > 0 {
            rx -= 1;
            rw += 1;
        }
        if y > 0 {
            ry -= 1;
            rh += 1;
        }
        if x < 340 {
            rw += 1;
        }
        if y < 261 {
            rh += 1;
        }

        drawing::rect(&mut self.canvas, rx, ry, rw, rh, outline_color);
        
        // the event dot
        self.canvas.put_pixel(x, y, color);
    }

    fn draw_event(&mut self, event: TrackedEvent) {
        match event.event_type {
            EventType::CpuRead{address, data: _, program_counter: _} => {
                self.draw_event_dot(event, cpu_register_color(address));
            },
            EventType::CpuWrite{address, data: _, program_counter: _} => {
                self.draw_event_dot(event, cpu_register_color(address));
            },
            EventType::CpuExecute{program_counter, data: _} => {
                self.draw_event_dot(event, cpu_register_color(program_counter));
            },
            _ => {}
        }
    }

    fn draw(&mut self, nes: &NesState) {
        // Clear!
        drawing::rect(&mut self.canvas, 0, 0, 341, 262, Color::rgb(50,50,50));

        // First, draw the current game screen, and a visualization of the electron beam
        for x in 0 .. 341 {
            for y in 0 .. 262 {
                let pixel_freshness = freshness(y as u16, x as u16, nes.ppu.current_scanline, nes.ppu.current_scanline_cycle);
                if x  > 0 && x <= 256 && y < 240 {
                    let palette_index = ((nes.ppu.screen[(y * 256 + x - 1) as usize]) as usize) * 3;
                    let color = Color::rgba(
                            NTSC_PAL[palette_index + 0],
                            NTSC_PAL[palette_index + 1],
                            NTSC_PAL[palette_index + 2],
                            192);
                    let scanline_freshness = (pixel_freshness.powf(32.0) * 255.0) as u8;
                    //let freshness8 = (scanline_freshness + cycle_freshness).min(255.0) as u8;
                    self.canvas.put_pixel(x, y, Color::rgb(scanline_freshness, scanline_freshness, scanline_freshness));
                    self.canvas.blend_pixel(
                        x, 
                        y,
                        color
                    );
                }

                let cycle_freshness = (pixel_freshness.powf(341.0 * 16.0) * 192.0) as u8;
                self.canvas.blend_pixel(
                    x, 
                    y,
                    Color::rgba(255, 255, 255, cycle_freshness)
                );
            }
        }

        // Next, draw every current event as a ... I dunno, a bright pixel I guess
        // We want all of the events from last frame *after* the current cycle
        for &event in nes.event_tracker.events_last_frame() {
            if event.scanline > nes.ppu.current_scanline ||
               (event.scanline == nes.ppu.current_scanline && event.cycle > nes.ppu.current_scanline_cycle) {
                self.draw_event(event);
            }
        }

        // We want all of the events from current frame *before* the current cycle
        for &event in nes.event_tracker.events_this_frame() {
            if event.scanline < nes.ppu.current_scanline ||
               (event.scanline == nes.ppu.current_scanline && event.cycle <= nes.ppu.current_scanline_cycle) {
                self.draw_event(event);
            }
        }

        // Now do the same loop, targeting the tooltip
        for &event in nes.event_tracker.events_last_frame() {
            if event.scanline > nes.ppu.current_scanline ||
               (event.scanline == nes.ppu.current_scanline && event.cycle > nes.ppu.current_scanline_cycle) {
                if self.tooltip_visible(event) {
                    self.draw_tooltip(event);
                }
            }
        }
        for &event in nes.event_tracker.events_this_frame() {
            if event.scanline < nes.ppu.current_scanline ||
               (event.scanline == nes.ppu.current_scanline && event.cycle <= nes.ppu.current_scanline_cycle) {
                if self.tooltip_visible(event) {
                    self.draw_tooltip(event);
                }
            }
        }

        // Draw the mouse position, which is useful because these event dots have *tiny* hitboxes
        self.canvas.blend_pixel(self.mx as u32, self.my as u32, Color::rgba(255, 255, 255, 192));
    }

    fn handle_move(&mut self, x: i32, y: i32) {
        self.mx = x;
        self.my = y;
    }
}

impl Panel for EventWindow {
    fn title(&self) -> &str {
        return "Event Viewer";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        match event {
            Event::RequestFrame => {self.draw(&runtime.nes)},
            Event::ShowEventWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},

            Event::MouseMove(x, y) => {self.handle_move(x, y);},
            _ => {}
        }
        return Vec::<Event>::new();
    }
    
    fn active_canvas(&self) -> &SimpleBuffer {
        return &self.canvas;
    }

    fn scale_factor(&self) -> u32 {
        return self.scale;
    }
}
