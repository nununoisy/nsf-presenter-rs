use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use rusticnes_core::nes::NesState;
use rusticnes_core::memory;

pub struct MemoryWindow {
    pub canvas: SimpleBuffer,
    pub counter: u8,
    pub font: Font,
    pub shown: bool,
    pub view_ppu: bool,
    pub memory_page: u16,
}

impl MemoryWindow {
    pub fn new() -> MemoryWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return MemoryWindow {
            canvas: SimpleBuffer::new(360, 220),
            counter: 0,
            font: font,
            shown: false,
            view_ppu: false,
            memory_page: 0x0000,
        };
    }

    pub fn draw_memory_page(&mut self, nes: &NesState, sx: u32, sy: u32) {
        for y in 0 .. 16 {
            for x in 0 .. 16 {
                let address = self.memory_page + (x as u16) + (y as u16 * 16);
                let byte: u8;
                let mut bg_color = Color::rgb(32, 32, 32);
                if (x + y) % 2 == 0 {
                    bg_color = Color::rgb(48, 48, 48);
                }
                if self.view_ppu {
                    let masked_address = address & 0x3FFF;
                    byte = nes.ppu.debug_read_byte(& *nes.mapper, masked_address);
                    if masked_address == (nes.ppu.current_vram_address & 0x3FFF) {
                        bg_color = Color::rgb(128, 32, 32);
                    } else if nes.ppu.recent_reads.contains(&masked_address) {
                        for i in 0 .. nes.ppu.recent_reads.len() {
                            if nes.ppu.recent_reads[i] == masked_address {
                                let brightness = 192 - (5 * i as u8);
                                bg_color = Color::rgb(64, brightness, 64);
                                break;
                            }
                        }
                    } else if nes.ppu.recent_writes.contains(&masked_address) {
                        for i in 0 .. nes.ppu.recent_writes.len() {
                            if nes.ppu.recent_writes[i] == masked_address {
                                let brightness = 192 - (5 * i as u8);
                                bg_color = Color::rgb(brightness, brightness, 32);
                                break;
                            }
                        }
                    }
                } else {
                    byte = memory::debug_read_byte(nes, address);
                    if address == nes.registers.pc {
                        bg_color = Color::rgb(128, 32, 32);
                    } else if address == (nes.registers.s as u16 + 0x100) {
                        bg_color = Color::rgb(32, 32, 128);
                    } else if nes.memory.recent_reads.contains(&address) {
                        for i in 0 .. nes.memory.recent_reads.len() {
                            if nes.memory.recent_reads[i] == address {
                                let brightness = 192 - (5 * i as u8);
                                bg_color = Color::rgb(64, brightness, 64);
                                break;
                            }
                        }
                    } else if nes.memory.recent_writes.contains(&address) {
                        for i in 0 .. nes.memory.recent_writes.len() {
                            if nes.memory.recent_writes[i] == address {
                                let brightness = 192 - (5 * i as u8);
                                bg_color = Color::rgb(brightness, brightness, 32);
                                break;
                            }
                        }
                    }
                }
                let mut text_color = Color::rgba(255, 255, 255, 192);
                if byte == 0 {
                    text_color = Color::rgba(255, 255, 255, 64);
                }
                let cell_x = sx + x * 19;
                let cell_y = sy + y * 11;
                drawing::rect(&mut self.canvas, cell_x, cell_y, 19, 11, bg_color);
                drawing::hex(&mut self.canvas, &self.font, 
                    cell_x + 2, cell_y + 2,
                    byte as u32, 2, 
                    text_color);
            }
        }
    }

    pub fn draw(&mut self, nes: &NesState) {
        let width = self.canvas.width;
        let height = self.canvas.height;
        
        drawing::rect(&mut self.canvas, 0, 0, width, 33, Color::rgb(0,0,0));
        drawing::rect(&mut self.canvas, 0, 0, 56, height, Color::rgb(0,0,0));
        drawing::text(&mut self.canvas, &self.font, 0, 0, &format!("{} Page: 0x{:04X}",
            if self.view_ppu {"PPU"} else {"CPU"}, self.memory_page), 
            Color::rgb(255, 255, 255));

        // Draw memory region selector
        for i in 0x0 .. 0x10 {
            // Highest Nybble
            let cell_x = 56  + (i as u32 * 19);
            let mut cell_y = 11;
            let mut text_color = Color::rgba(255, 255, 255, 64);
            if ((self.memory_page & 0xF000) >> 12) == i {
                drawing::rect(&mut self.canvas, cell_x, cell_y, 19, 11, Color::rgb(64, 64, 64));
                text_color = Color::rgba(255, 255, 255, 192);
            }
            drawing::hex(&mut self.canvas, &self.font, cell_x + 2, cell_y + 2, i as u32, 1, text_color);
            drawing::char(&mut self.canvas, &self.font, cell_x + 2 + 8, cell_y + 2, 'X', text_color);

            // Second-highest Nybble
            text_color = Color::rgba(255, 255, 255, 64);
            cell_y = 22;
            if ((self.memory_page & 0x0F00) >> 8) == i {
                drawing::rect(&mut self.canvas, cell_x, cell_y, 19, 11, Color::rgb(64, 64, 64));
                text_color = Color::rgba(255, 255, 255, 192);
            }
            drawing::char(&mut self.canvas, &self.font, cell_x + 2, cell_y + 2, 'X', text_color);
            drawing::hex(&mut self.canvas, &self.font, cell_x + 2 + 8, cell_y + 2, i as u32, 1, text_color);
        }

        // Draw row labels
        for i in 0 .. 0x10 {
            drawing::text(&mut self.canvas, &self.font, 0, 44 + 2 + (i as u32 * 11), &format!("0x{:04X}",
                self.memory_page + (i as u16 * 0x10)), 
                Color::rgba(255, 255, 255, 64));
        }
        self.draw_memory_page(nes, 56, 44);
    }

    pub fn handle_click(&mut self, mx: i32, my: i32) {
        if my < 11 && mx < 32 {
            self.view_ppu = !self.view_ppu;
        }
        if my >= 11 && my < 22 && mx > 56 && mx < 360 {
            let high_nybble = ((mx - 56) / 19) as u16;
            self.memory_page = (self.memory_page & 0x0FFF) | (high_nybble << 12);
        }
        if my >= 22 && my < 33 && mx > 56 && mx < 360 {
            let low_nybble = ((mx - 56) / 19) as u16;
            self.memory_page = (self.memory_page & 0xF0FF) | (low_nybble << 8);
        }
    }
}


impl Panel for MemoryWindow {
    fn title(&self) -> &str {
        return "Memory Viewer";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        match event {
            Event::RequestFrame => {self.draw(&runtime.nes)},
            Event::ShowMemoryWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},
            Event::MemoryViewerNextPage => {
                self.memory_page = self.memory_page.wrapping_add(0x100);
            },
            Event::MemoryViewerPreviousPage => {
                self.memory_page = self.memory_page.wrapping_sub(0x100);
            },
            Event::MemoryViewerNextBus => {
                self.view_ppu = !self.view_ppu;
            },
            Event::MouseClick(x, y) => {self.handle_click(x, y);},
            _ => {}
        }
        return Vec::<Event>::new();
    }
    
    fn active_canvas(&self) -> &SimpleBuffer {
        return &self.canvas;
    }

    fn scale_factor(&self) -> u32 {
        return 2;
    }
}