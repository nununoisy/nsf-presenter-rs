use application::RuntimeState;
use drawing;
use drawing::Color;
use drawing::Font;
use drawing::SimpleBuffer;
use events::Event;
use panel::Panel;

use rusticnes_core::nes::NesState;
use rusticnes_core::opcode_info::disassemble_instruction;
use rusticnes_core::memory;

pub struct CpuWindow {
    pub canvas: SimpleBuffer,
    pub font: Font,
    pub shown: bool,
}

impl CpuWindow {
    pub fn new() -> CpuWindow {
        let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

        return CpuWindow {
            canvas: SimpleBuffer::new(256, 300),
            font: font,
            shown: false,
        };
    }

    pub fn draw_registers(&mut self, nes: &NesState, x: u32, y: u32) {
        drawing::text(&mut self.canvas, &self.font, x, y, 
            "===== Registers =====", 
            Color::rgb(192, 192, 192));
        drawing::text(&mut self.canvas, &self.font, x, y + 8, 
            &format!("A: 0x{:02X}", nes.registers.a), Color::rgb(255, 255, 128));
        drawing::text(&mut self.canvas, &self.font, x, y + 16, 
            &format!("X: 0x{:02X}", nes.registers.x), Color::rgb(160, 160, 160));
        drawing::text(&mut self.canvas, &self.font, x, y + 24, 
            &format!("Y: 0x{:02X}", nes.registers.y), Color::rgb(224, 224, 224));

        drawing::text(&mut self.canvas, &self.font, x + 64, y + 8, 
            &format!("PC: 0x{:04X}", nes.registers.pc), Color::rgb(255, 128, 128));
        drawing::text(&mut self.canvas, &self.font, x + 64, y + 16, 
            &format!("S:      {:02X}", nes.registers.s), Color::rgb(128, 128, 255));
        drawing::text(&mut self.canvas, &self.font, x + 64, y + 16, 
                     "    0x10  ",                       Color::rgb(128, 128, 255));
        drawing::text(&mut self.canvas, &self.font, x + 64, y + 24, 
            "F:  nvdzic", Color::rgba(128, 192, 128, 64));
        drawing::text(&mut self.canvas, &self.font, x + 64, y + 24, 
            &format!("F:  {}{}{}{}{}{}",
                if nes.registers.flags.negative            {"n"} else {" "},
                if nes.registers.flags.overflow            {"v"} else {" "},
                if nes.registers.flags.decimal             {"d"} else {" "},
                if nes.registers.flags.zero                {"z"} else {" "},
                if nes.registers.flags.interrupts_disabled {"i"} else {" "},
                if nes.registers.flags.carry               {"c"} else {" "}),
            Color::rgb(128, 192, 128));
    }

    pub fn draw_disassembly(&mut self, nes: &NesState, x: u32, y: u32) {
        drawing::text(&mut self.canvas, &self.font, x, y, 
        "===== Disassembly =====", Color::rgb(255, 255, 255));

        let mut data_bytes_to_skip = 0;
        for i in 0 .. 30 {
            let pc = nes.registers.pc + (i as u16);
            let opcode = memory::debug_read_byte(nes, pc);
            let data1 = memory::debug_read_byte(nes, pc + 1);
            let data2 = memory::debug_read_byte(nes, pc + 2);
            let (instruction, data_bytes) = disassemble_instruction(opcode, data1, data2);
            let mut text_color = Color::rgb(255, 255, 255);

            if data_bytes_to_skip > 0 {
                text_color = Color::rgb(64, 64, 64);
                data_bytes_to_skip -= 1;
            } else {
                data_bytes_to_skip = data_bytes;
            }

            drawing::text(&mut self.canvas, &self.font, x, y + 16 + (i as u32 * 8),
                &format!("0x{:04X} - 0x{:02X}:  {}", pc, opcode, instruction),
                text_color);
        }
    }

    fn draw(&mut self, nes: &NesState) {
        // Clear!
        let width = self.canvas.width;
        let height = self.canvas.height;
        drawing::rect(&mut self.canvas, 0, 0, width, height, Color::rgb(0,0,0));
        self.draw_registers(nes, 0, 0);
        self.draw_disassembly(nes, 0, 40);    
    }
}

impl Panel for CpuWindow {
    fn title(&self) -> &str {
        return "CPU Status";
    }

    fn shown(&self) -> bool {
        return self.shown;
    }

    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event> {
        match event {
            Event::RequestFrame => {self.draw(&runtime.nes)},
            Event::ShowCpuWindow => {self.shown = true},
            Event::CloseWindow => {self.shown = false},
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