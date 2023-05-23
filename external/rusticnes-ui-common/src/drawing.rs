use image;
use image::Pixel;
use image::RgbaImage;

use csscolorparser;

fn blend_component(a: u8, b: u8, alpha: u8) -> u8 {
    return (
        (a as u16 * (255 - alpha as u16) / 255) + 
        (b as u16 * (alpha as u16) / 255)
    ) as u8;
}

#[derive(Copy,Clone)]
pub struct Color {
    pub data: [u8; 4]
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        return Color {
            data: [r, g, b, 255]
        }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        return Color {
            data: [r, g, b, a]
        }   
    }

    pub fn from_raw(argb: u32) -> Color {
        return Color {
            data: [
                ((argb & 0x00FF0000) >> 16) as u8,
                ((argb & 0x0000FF00) >> 8)  as u8,
                ((argb & 0x000000FF) >> 0)  as u8,
                ((argb & 0xFF000000) >> 24) as u8
            ]
        }
    }

    pub fn from_slice(color_data: &[u8]) -> Color {
        return Color {
            data: [
                color_data[0],
                color_data[1],
                color_data[2],
                color_data[3]
            ]
        }
    }

    pub fn from_string(color_string: &str) -> Result<Color, String> {
        match csscolorparser::parse(color_string) {
            Ok(css_color) => {
                let rgba8_color = css_color.to_rgba8();
                return Ok(Color {
                    data: [
                        rgba8_color[0],
                        rgba8_color[1],
                        rgba8_color[2],
                        rgba8_color[3]
                    ]
                });        
            },
            Err(e) => {
                return Err(e.to_string());
            }
        }
    }

    pub fn r(&self) -> u8 {
        return self.data[0];
    }

    pub fn g(&self) -> u8 {
        return self.data[1];
    }

    pub fn b(&self) -> u8 {
        return self.data[2];
    }

    pub fn alpha(&self) -> u8 {
        return self.data[3];
    }

    pub fn set_alpha(&mut self, a: u8) {
        self.data[3] = a;
    }
}

pub fn apply_gradient(colors: Vec<Color>, index: f32) -> Color {
    if colors.len() == 0 {
        return Color::rgb(0,0,0);
    }
    if colors.len() == 1  {
        return colors[0];
    }

    let lower_index = (index * (colors.len() as f32)).floor().max(0.0).min((colors.len() - 1) as f32) as usize;
    let upper_index = (index * (colors.len() as f32)).ceil().max(0.0).min((colors.len() - 1) as f32) as usize;
    let weight = ((index * (colors.len() as f32)).fract() * 255.0) as u8;


    let final_color = Color::rgb(
        blend_component(colors[lower_index].r(), colors[upper_index].r(), weight),
        blend_component(colors[lower_index].g(), colors[upper_index].g(), weight),
        blend_component(colors[lower_index].b(), colors[upper_index].b(), weight),
    );
    return final_color;

}

#[derive(Clone)]
pub struct SimpleBuffer {
    pub buffer: Vec<u8>,
    pub width: u32,
    pub height: u32,
}



impl SimpleBuffer {
    pub fn new(width: u32, height: u32) -> SimpleBuffer {
        return SimpleBuffer{
            width: width,
            height: height,
            buffer: vec!(0u8; (width * height * 4) as usize)
        }
    }

    pub fn from_image(img: RgbaImage) -> SimpleBuffer {
        let (img_width, img_height) = img.dimensions();
        let mut raw_buffer = SimpleBuffer::new(img_width, img_height);
        for x in 0 .. img_width {
            for y in 0 .. img_height {
                let pixel = img[(x, y)].to_rgba();
                let color = Color::from_slice(&pixel.data);
                raw_buffer.put_pixel(x, y, color);
            }
        }
        return raw_buffer
    }

    pub fn from_raw(bitmap_data: &[u8]) -> SimpleBuffer {
        let img = image::load_from_memory(bitmap_data).unwrap().to_rgba();
        return SimpleBuffer::from_image(img);
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, color: Color) {
        let index = ((y * self.width + x) * 4) as usize;
        self.buffer[index .. (index + 4)].copy_from_slice(&color.data);
    }

    pub fn blend_pixel_old(&mut self, x: u32, y: u32, color: Color) {
        let index = ((y * self.width + x) * 4) as usize;
        let original = self.get_pixel(x, y);
        let r = blend_component(original.r(), color.r(), color.alpha());
        let g = blend_component(original.g(), color.g(), color.alpha());
        let b = blend_component(original.b(), color.b(), color.alpha());
        self.buffer[index .. (index + 4)].copy_from_slice(&[r, g, b, 255]);
    }

    pub fn blend_pixel(&mut self, x: u32, y: u32, color: Color) {
        let index = ((y * self.width + x) * 4) as usize;
        let original = self.get_pixel(x, y);

        // avoid division by zero
        if color.alpha() == 0 {
            return; // do nothing!
        }

        let alpha_new = (color.alpha() as f32) / 255.0;
        let remaining_potential_weight = 1.0 - alpha_new;
        let alpha_original = ((original.alpha() as f32) / 255.0) * remaining_potential_weight;
        let total_alpha = alpha_new + alpha_original;

        let new_color_weight = alpha_new / total_alpha;
        let old_color_weight = alpha_original / total_alpha;

        let r = ((original.r() as f32) * old_color_weight + (color.r() as f32) * new_color_weight).min(255.0) as u8;
        let g = ((original.g() as f32) * old_color_weight + (color.g() as f32) * new_color_weight).min(255.0) as u8;
        let b = ((original.b() as f32) * old_color_weight + (color.b() as f32) * new_color_weight).min(255.0) as u8;
        let alpha = (total_alpha.min(1.0) * 255.0) as u8;

        self.buffer[index .. (index + 4)].copy_from_slice(&[r, g, b, alpha]);
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        let index = ((y * self.width + x) * 4) as usize;
        return Color::rgba(
            self.buffer[index],
            self.buffer[index + 1],
            self.buffer[index + 2],
            self.buffer[index + 3]
        );
    }
}

pub struct Font {
    pub glyph_width: u32,
    pub glyphs: Vec<SimpleBuffer>,
}

impl Font {
    pub fn from_image(img: RgbaImage, glyph_width: u32) -> Font {
        let raw_buffer = SimpleBuffer::from_image(img);

        // Convert each individual character into its own glyph:
        let mut glyphs = vec!(SimpleBuffer::new(glyph_width, raw_buffer.height); 128 - 32);
        for i in 0 .. (128 - 32) {
            for y in 0 .. raw_buffer.height {
                for x in 0 .. glyph_width {
                    glyphs[i].put_pixel(x, y, raw_buffer.get_pixel((i as u32) * glyph_width + (x as u32), y as u32));
                }
            }
        }

        return Font {
            glyph_width: glyph_width,
            glyphs: glyphs,
        }
    }
    pub fn from_raw(bitmap_data: &[u8], glyph_width: u32) -> Font {
        let img = image::load_from_memory(bitmap_data).unwrap().to_rgba();
        return Font::from_image(img, glyph_width);
    }
}

pub fn blit(destination: &mut SimpleBuffer, source: &SimpleBuffer, dx: u32, dy: u32, color: Color) {
    for x in 0 .. source.width {
        for y in 0 .. source.height {
            let mut source_color = source.get_pixel(x, y);
            let destination_color = destination.get_pixel(dx + x, dy + y);
            // Multiply by target color
            for i in 0 .. 4 {
                source_color.data[i] = ((source_color.data[i] as u16 * color.data[i] as u16) / 255) as u8;
            }
            // Blend to apply alpha transparency
            let source_alpha = source_color.alpha() as u16;
            let destination_alpha = 255 - source_alpha;
            let final_color = Color::rgb(
                ((destination_color.r() as u16 * destination_alpha + source_color.r() as u16 * source_alpha) / 255) as u8,
                ((destination_color.g() as u16 * destination_alpha + source_color.g() as u16 * source_alpha) / 255) as u8,
                ((destination_color.b() as u16 * destination_alpha + source_color.b() as u16 * source_alpha) / 255) as u8
            );
            destination.put_pixel(dx + x, dy + y, final_color);
        }
    }
}

pub fn char(destination: &mut SimpleBuffer, font: &Font, x: u32, y: u32, c: char, color: Color) {
    if c.is_ascii() {
        let ascii_code_point = c as u32;
        if ascii_code_point >= 32 && ascii_code_point < 127 {
            blit(destination, &font.glyphs[(ascii_code_point - 32) as usize], x, y, color);
        }
    }
}

pub fn text(destination: &mut SimpleBuffer, font: &Font, x: u32, y: u32, s: &str, color: Color) {
    for i in 0 .. s.len() {
        char(destination, font, x + ((i as u32) * font.glyph_width), y, s.chars().nth(i).unwrap(), color);
    }
}

pub fn hex(destination: &mut SimpleBuffer, font: &Font, x: u32, y: u32, value: u32, nybbles: u32, color: Color) {
    let char_map = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'];
    for i in 0 .. nybbles {
        let c = char_map[((value & (0xF << (i * 4))) >> (i * 4)) as usize];
        char(destination, font, x + (nybbles - 1 - (i as u32)) * font.glyph_width, y, c, color);
    }
}

pub fn rect(destination: &mut SimpleBuffer, x: u32, y: u32, width: u32, height: u32, color: Color) {
    for dx in x .. (x + width) {
        for dy in y .. (y + height) {
            destination.put_pixel(dx, dy, color);
        }
    }
}

pub fn blend_rect(destination: &mut SimpleBuffer, x: u32, y: u32, width: u32, height: u32, color: Color) {
    for dx in x .. (x + width) {
        for dy in y .. (y + height) {
            destination.blend_pixel(dx, dy, color);
        }
    }
}