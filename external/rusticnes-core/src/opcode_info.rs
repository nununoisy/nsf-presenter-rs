pub fn alu_block(addressing_mode_index: u8, opcode_index: u8) -> (&'static str, &'static str) {
  let addressing_mode = match addressing_mode_index {
    // Zero Page Mode
    0b000 => "(d, x)",
    0b001 => "d",
    0b010 => "#i",
    0b011 => "a",
    0b100 => "(d), y",
    0b101 => "d, x",
    0b110 => "a, y",
    0b111 => "a, x",

    // Not implemented yet
    _ => "???",
  };

  let opcode_name = match opcode_index {
    0b000 => "ORA",
    0b001 => "AND",
    0b010 => "EOR",
    0b011 => "ADC",
    0b100 => "STA",
    0b101 => "LDA",
    0b110 => "CMP",
    0b111 => "SBC",
    _ => "???"
  };

  return (opcode_name, addressing_mode);
}

pub fn rmw_block(opcode: u8, addressing_mode_index: u8, opcode_index: u8) -> (&'static  str, &'static str) {
  // First, handle some block 10 opcodes that break the mold
  return match opcode {
    // Assorted NOPs
    0x82 | 0xC2 | 0xE2 => ("NOP", "#i"),
    0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xEA | 0xFA => ("NOP", ""),
    // Certain opcodes may be vital to your success. THESE opcodes are not.
    0x02 | 0x22 | 0x42 | 0x62 | 0x12 | 0x32 | 0x52 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => ("STP", ""),
    0xA2 => ("LDX", "#i"),
    0x8A => ("TXA", ""),
    0xAA => ("TAX", ""),
    0xCA => ("DEX", ""),
    0x9A => ("TXS", ""),
    0xBA => ("TSX", ""),
    0x96 => ("STX", "d, y"),
    0xB6 => ("LDX", "d, y"),
    0xBE => ("LDX", "a, y"),
    _ => {
      let addressing_mode = match addressing_mode_index {
        // Zero Page Mode
        0b001 => "d",
        0b010 => "", // Note: masked for 8A, AA, CA, and EA above
        0b011 => "a",
        0b101 => "d, x",
        0b111 => "a, x",

        // Not implemented yet
        _ => "???",
      };

      let opcode_name =  match opcode_index {
        0b000 => "ASL",
        0b001 => "ROL",
        0b010 => "LSR",
        0b011 => "ROR",
        0b100 => "STX",
        0b101 => "LDX",
        0b110 => "DEC",
        0b111 => "INC",
        _ => "???"
      };

      return (opcode_name, addressing_mode);
    }
  };
}

pub fn control_block(opcode: u8) -> (&'static str, &'static str) {
  // Everything is pretty irregular, so we'll just match the whole opcode
  return match opcode {
  	0x10 => ("BPL", ""),
  	0x30 => ("BMI", ""),
  	0x50 => ("BVC", ""),
  	0x70 => ("BVS", ""),
  	0x90 => ("BCC", ""),
  	0xB0 => ("BCS", ""),
  	0xD0 => ("BNE", ""),
  	0xF0 => ("BEQ", ""),

    0x00 => ("BRK", ""),
    0x80 => ("NOP", "#i"),

    // Opcodes with similar addressing modes
    0xA0 => ("LDY", "#i"),
    0xC0 => ("CPY", "#i"),
    0xE0 => ("CPX", "#i"),
    0x24 => ("BIT", "d"),
    0x84 => ("STY", "d"),
    0xA4 => ("LDY", "d"),
    0xC4 => ("CPY", "d"),
    0xE4 => ("CPX", "d"),
    0x2C => ("BIT", "a"),
    0x8C => ("STY", "a"),
    0xAC => ("LDY", "a"),
    0xCC => ("CPY", "a"),
    0xEC => ("CPX", "a"),
    0x94 => ("STY", "d, x"),
    0xB4 => ("LDY", "d, x"),
    0xBC => ("LDY", "a, x"),

    0x4C => ("JMP", "a"),
    0x6C => ("JMP", "(a)"),

    0x08 => ("PHP", ""),
    0x28 => ("PLP", ""),
    0x48 => ("PHA", ""),
    0x68 => ("PLA", ""),

    0x20 => ("JSR", ""),
    0x40 => ("RTI", ""),
    0x60 => ("RTS", ""),

    0x88 => ("DEY", ""),
    0xA8 => ("TAY", ""),
    0xC8 => ("INY", ""),
    0xE8 => ("INX", ""),

    0x18 => ("CLC", ""),
    0x58 => ("CLI", ""),
    0xB8 => ("CLV", ""),
    0xD8 => ("CLD", ""),
    0x38 => ("SEC", ""),
    0x78 => ("SEI", ""),
    0xF8 => ("SED", ""),
    0x98 => ("TYA", ""),

    _ => ("???", "???")
  };
}

pub fn addressing_bytes(addressing_mode: &str) -> u8 {
	return match addressing_mode {
		"#i" | "d" | "(d, x)" | "(d), y" | "d, x"  => 1,
		"a" | "a, x" | "a, y" | "(a)" => 2,
		_ => 0
	}
}

pub fn disassemble_instruction(opcode: u8, _: u8, _: u8) -> (String, u8) {
  let logic_block = opcode & 0b0000_0011;
  let addressing_mode_index = (opcode & 0b0001_1100) >> 2;
  let opcode_index = (opcode & 0b1110_0000) >> 5;

  let (opcode_name, addressing_mode) = match logic_block {
    0b00 => control_block(opcode),
    0b01 => alu_block(addressing_mode_index, opcode_index),
    0b10 => rmw_block(opcode, addressing_mode_index, opcode_index),
    _ => ("???", "")
  };

  let instruction = format!("{} {}", opcode_name, addressing_mode);
  let data_bytes = addressing_bytes(addressing_mode);
  return (instruction, data_bytes);
}