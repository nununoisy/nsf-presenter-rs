use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use rusticnes_core::mmc::mapper::Mapper;
use rusticnes_core::nes::NesState;
use rusticnes_core::ppu;
use rusticnes_core::palettes::NTSC_PAL;

fn draw_tile(mapper: &dyn Mapper, pattern_address: u16, tile_index: u16, buffer: &mut SimpleBuffer, dx: u32, dy: u32, palette: &[u8]) {
    for py in 0 .. 8 {
        let tile_address = pattern_address + tile_index * 16 + py;
        let mut tile_low  = mapper.debug_read_ppu(tile_address).unwrap_or(0);
        let mut tile_high = mapper.debug_read_ppu(tile_address + 8).unwrap_or(0);
        for px in 0 .. 8 {
            let palette_index = (tile_low & 0x1) + ((tile_high & 0x1) << 1);
            tile_low = tile_low >> 1;
            tile_high = tile_high >> 1;
            buffer.put_pixel(
                dx + (7 - px as u32), 
                dy + (py as u32), 
                Color::rgb(
                    palette[(palette_index * 4 + 0) as usize],
                    palette[(palette_index * 4 + 1) as usize],
                    palette[(palette_index * 4 + 2) as usize])
            );
        }
    }
}

fn draw_2x_tile(mapper: &dyn Mapper, pattern_address: u16, tile_index: u16, buffer: &mut SimpleBuffer, dx: u32, dy: u32, palette: &[u8]) {
    for py in 0 .. 8 {
        let tile_address = pattern_address + tile_index * 16 + py;
        let mut tile_low  = mapper.debug_read_ppu(tile_address).unwrap_or(0);
        let mut tile_high = mapper.debug_read_ppu(tile_address + 8).unwrap_or(0);
        for px in 0 .. 8 {
            let palette_index = (tile_low & 0x1) + ((tile_high & 0x1) << 1);
            tile_low = tile_low >> 1;
            tile_high = tile_high >> 1;
            for sx in 0 .. 2 {
                for sy in 0 .. 2 {
                    buffer.put_pixel(
                        dx + (7 - px as u32) * 2 + sx, 
                        dy + (py as u32) * 2 + sy, 
                        Color::rgb(
                            palette[(palette_index * 4 + 0) as usize],
                            palette[(palette_index * 4 + 1) as usize],
                            palette[(palette_index * 4 + 2) as usize])
                    );
                }
            }
        }
    }
}

fn generate_chr_pattern(mapper: &dyn Mapper, pattern_address: u16, buffer: &mut SimpleBuffer, dx: u32, dy: u32) {
    let debug_palette: [u8; 4*4] = [
        255, 255, 255, 255,
        192, 192, 192, 255,
        128, 128, 128, 255,
          0,   0,   0, 255];

    for x in 0 .. 16 {
        for y in 0 .. 16 {
            let tile_index = y * 16 + x;
            draw_tile(mapper, pattern_address, tile_index as u16, buffer, 
                      dx + x * 8, dy + y * 8, &debug_palette);
        }
    }
}

fn draw_color_box(buffer: &mut SimpleBuffer, dx: u32, dy: u32, color: Color) {
    // First, draw a white outline
    for x in 0 .. 16 {
        for y in 0 .. 16 {
            buffer.put_pixel(dx + x, dy + y, Color::rgb(255, 255, 255));
        }
    }
    // Then draw the palette color itself in the center of the outline
    for x in 1 .. 15 {
        for y in 1 .. 15 {
            buffer.put_pixel(dx + x, dy + y, color);
        }
    }
}

pub struct PpuWindow {
    pub canvas: SimpleBuffer,
    pub palette_cache: [[u8; 4*4]; 4*2],
    pub font: Font,
    pub shown: bool,
}

impl PpuWindow {
    pub fn new() -> PpuWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return PpuWindow {
            canvas: SimpleBuffer::new(792, 512),
            palette_cache: [[0u8; 4*4]; 4*2],
            font: font,
            shown: false,
        }
    }

    pub fn update_palette_cache(&mut self, nes: &NesState) {
        // Initialize all palette colors with a straight copy
        for p in 0 .. 8 {
            for i in 0 .. 4 {
                let palette_color = nes.ppu.debug_read_byte(& *nes.mapper, 0x3F00 + p * 4 + i) as usize * 3;
                self.palette_cache[p as usize][i as usize * 4 + 0] = NTSC_PAL[palette_color + 0];
                self.palette_cache[p as usize][i as usize * 4 + 1] = NTSC_PAL[palette_color + 1];
                self.palette_cache[p as usize][i as usize * 4 + 2] = NTSC_PAL[palette_color + 2];
                self.palette_cache[p as usize][i as usize * 4 + 3] = 255;
            }
        }

        // Override the background colors with the universal background color:
        for p in 1 .. 8 {
            self.palette_cache[p][0] = self.palette_cache[0][0];
            self.palette_cache[p][1] = self.palette_cache[0][1];
            self.palette_cache[p][2] = self.palette_cache[0][2];
            self.palette_cache[p][3] = 255;
        }
    }

    pub fn generate_nametables(&mut self, mapper: &dyn Mapper, ppu: &ppu::PpuState, dx: u32, dy: u32) {
        let mut pattern_address = 0x0000;
        if (ppu.control & 0x10) != 0 {
            pattern_address = 0x1000;
        }
        
        for tx in 0 .. 64 {
            for ty in 0 .. 60 {
                let tile_index = ppu.get_bg_tile(mapper, tx, ty);
                let palette_index = ppu.get_bg_palette(mapper, tx, ty);
                draw_tile(mapper, pattern_address, tile_index as u16, &mut self.canvas, 
                    dx + tx as u32 * 8, dy + ty as u32 * 8, &self.palette_cache[palette_index as usize]);
            }
        }
    
        // Draw a red border around the present scroll viewport
        let vram_address = ppu.current_vram_address;
        let coarse_x =  vram_address & 0b000_00_00000_11111;
        let coarse_y = (vram_address & 0b000_00_11111_00000) >> 5;
        let fine_x = ppu.fine_x;
        let fine_y =   (vram_address & 0b111_00_00000_00000) >> 12;
        let scroll_x = (coarse_x << 3 | fine_x as u16) as u32;
        let scroll_y = (coarse_y << 3 | fine_y as u16) as u32;

        for x in scroll_x .. scroll_x + 256 {
            let px = x % 512;
            let mut py = (scroll_y) % 480;
            self.canvas.put_pixel(dx + px, dy + py, Color::rgb(255, 0, 0));
            py = (scroll_y + 239) % 480;
            self.canvas.put_pixel(dx + px, dy + py, Color::rgb(255, 0, 0));
        }

        for y in scroll_y .. scroll_y + 240 {
            let py = y % 480;
            let mut px = scroll_x % 512;
            self.canvas.put_pixel(dx + px, dy + py, Color::rgb(255, 0, 0));
            px = (scroll_x + 255) % 512;
            self.canvas.put_pixel(dx + px, dy + py, Color::rgb(255, 0, 0));
        }
    }

    pub fn draw_palettes(&mut self, dx: u32, dy: u32) {
        // Global Background (just once)
        let color = Color::from_slice(&self.palette_cache[0][0 .. 4]);
        draw_color_box(&mut self.canvas, dx, dy, color);

        // Backgrounds
        for p in 0 .. 4 {
            for i in 1 .. 4 {
                let x = dx + p * 64 + i * 15;
                let y = dy;
                let color = Color::from_slice(&self.palette_cache[p as usize][(i * 4) as usize .. (i * 4 + 4) as usize]);
                draw_color_box(&mut self.canvas, x, y, color);
            }
        }

        // Sprites
        for p in 0 .. 4 {
            for i in 1 .. 4 {
                let x = dx + p * 64 + i * 15;
                let y = dy + 18;
                let color = Color::from_slice(&self.palette_cache[(p + 4) as usize][(i * 4) as usize .. (i * 4 + 4) as usize]);
                draw_color_box(&mut self.canvas, x, y, color);
            }
        }
    }

    pub fn draw_sprites(&mut self, nes: &NesState, dx: u32, dy: u32) {
        let mut sprite_size = 8;
        if (nes.ppu.control & 0b0010_0000) != 0 {
            sprite_size = 16;
        }

        for x in 0 .. 8 {
            for y in 0 .. 8 {
                let sprite_index = y * 8 + x;
                let sprite_y =     nes.ppu.oam[sprite_index * 4 + 0];
                let sprite_tile =  nes.ppu.oam[sprite_index * 4 + 1];
                let sprite_flags = nes.ppu.oam[sprite_index * 4 + 2];
                let sprite_x =     nes.ppu.oam[sprite_index * 4 + 3];
                
                let palette_index = sprite_flags & 0b0000_0011;
                let mut pattern_address: u16 = 0x0000;

                let cell_width = 35;
                let cell_height = 40;
                let cell_x = dx + x as u32 * cell_width;
                let cell_y = dy + y as u32 * cell_height;

                // If we're using 8x16 sprites, set the pattern based on the sprite's tile index
                if sprite_size == 16 {
                    if (sprite_tile & 0b1) != 0 {
                        pattern_address = 0x1000;
                    }
                    let large_sprite_tile = sprite_tile & 0b1111_1110;

                    drawing::rect(&mut self.canvas, 
                        cell_x, cell_y,
                        18, 34, 
                        Color::rgb(255, 255, 255));
                    draw_2x_tile(& *nes.mapper, pattern_address, large_sprite_tile as u16, &mut self.canvas, 
                        cell_x + 1, cell_y + 1,
                        &self.palette_cache[(palette_index + 4) as usize]);
                    draw_2x_tile(& *nes.mapper, pattern_address, (large_sprite_tile + 1) as u16, &mut self.canvas, 
                        cell_x + 1, cell_y + 1 + 16,
                        &self.palette_cache[(palette_index + 4) as usize]);
                } else {
                    // Otherwise, the pattern is selected by PPUCTL
                    if (nes.ppu.control & 0b0000_1000) != 0 {
                        pattern_address = 0x1000;
                    }

                    drawing::rect(&mut self.canvas, 
                        cell_x, cell_y,
                        18, 18, 
                        Color::rgb(255, 255, 255));
                    draw_2x_tile(& *nes.mapper, pattern_address, sprite_tile as u16, &mut self.canvas, 
                        cell_x + 1, cell_y + 1,
                        &self.palette_cache[(palette_index + 4) as usize]);
                }

                let text_color = Color::rgb(255, 255, 255);
                let bg_color = Color::rgb(0, 0, 0);

                drawing::rect(&mut self.canvas, 
                    cell_x + 19, cell_y, 
                    16, 32, bg_color);
                drawing::hex(&mut self.canvas, &self.font, cell_x + 19, cell_y + 0,
                    sprite_y as u32, 2, text_color);
                drawing::hex(&mut self.canvas, &self.font, cell_x + 19, cell_y + 8,
                    sprite_tile as u32, 2, text_color);
                drawing::hex(&mut self.canvas, &self.font, cell_x + 19, cell_y + 16,
                    sprite_flags as u32, 2, text_color);
                drawing::hex(&mut self.canvas, &self.font, cell_x + 19, cell_y + 24,
                    sprite_x as u32, 2, text_color);
            }
        }
    }

    fn update(&mut self, nes: &NesState) {
        self.update_palette_cache(nes);
    }

    fn draw(&mut self, nes: &NesState) {
        // Left Pane: CHR memory, Palette Colors
        generate_chr_pattern(& *nes.mapper, 0x0000, &mut self.canvas,   8, 0);
        generate_chr_pattern(& *nes.mapper, 0x1000, &mut self.canvas, 144, 0);
        self.draw_palettes(14, 130);
        self.draw_sprites(nes, 0, 170);
        // Right Panel: Entire nametable
        self.generate_nametables(& *nes.mapper, &nes.ppu, 280, 0);
    }
}

impl Panel for PpuWindow {
    fn title(&self) -> &str {
        return "PPU";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        match event {
            Event::Update => {self.update(&runtime.nes)},
            Event::RequestFrame => {self.draw(&runtime.nes)},
            Event::ShowPpuWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},
            _ => {}
        }
        return Vec::<Event>::new();
    }
    
    fn active_canvas(&self) -> &SimpleBuffer {
        return &self.canvas;
    }
}