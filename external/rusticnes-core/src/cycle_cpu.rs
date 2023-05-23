// Documentation for this 6502 implementation came from many sources, but the following
// two guides served as the primary inspiration:
// http://www.llx.com/~nparker/a2/opcodes.html - For opcode decoding structure
// http://nesdev.com/6502_cpu.txt - for information on cycle timings for each addressing mode

use addressing;
use memory::read_byte;
use memory::write_byte;
use nes::NesState;
use opcodes;
use unofficial_opcodes;

#[derive(Copy, Clone)]
pub struct Flags {
    pub carry: bool,
    pub zero: bool,
    pub decimal: bool,
    pub interrupts_disabled: bool,
    pub overflow: bool,
    pub negative: bool,

    // internal only
    pub last_nmi: bool,
}

#[derive(Copy, Clone)]
pub struct Registers {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub s: u8,
    pub flags: Flags,
}

impl Registers {
    pub fn new() -> Registers {
        return Registers {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0,
            flags: Flags {
                carry: false,
                zero: false,
                interrupts_disabled: false,
                decimal: false,
                overflow: false,
                negative: false,
                last_nmi: false,
            }
        }
    }

    pub fn status_as_byte(&self, s_flag: bool) -> u8 {
        return (self.flags.carry     as u8) +
               ((self.flags.zero      as u8) << 1) +
               ((self.flags.interrupts_disabled as u8) << 2) +
               ((self.flags.decimal   as u8) << 3) +
               ((s_flag                    as u8) << 4) +
               ((1u8                            ) << 5) + // always set
               ((self.flags.overflow  as u8) << 6) +
               ((self.flags.negative  as u8) << 7)
    }

    pub fn set_status_from_byte(&mut self, data: u8) {
        self.flags.carry =    data & (1 << 0) != 0;
        self.flags.zero =     data & (1 << 1) != 0;
        self.flags.interrupts_disabled = data & (1 << 2) != 0;
        self.flags.decimal =  data & (1 << 3) != 0;
        // bits 4 and 5, the s_flag, do not actually exist
        self.flags.overflow = data & (1 << 6) != 0;
        self.flags.negative = data & (1 << 7) != 0;
    }
}

pub struct CpuState {
  pub tick: u8,
  pub opcode: u8,
  pub data1: u8,
  pub data2: u8,
  pub temp_address: u16,
  pub service_routine_active: bool,
  pub nmi_requested: bool,
  pub irq_requested: bool,
  pub last_nmi: bool,
  pub upcoming_write: bool,

  pub oam_dma_active: bool,
  pub oam_dma_cycle: u16,
  pub oam_dma_address: u16,
}

impl CpuState {
  pub fn new() -> CpuState{
    return CpuState {
      tick: 0,
      opcode: 0,
      data1: 0,
      data2: 0,
      temp_address: 0,
      service_routine_active: false,
      nmi_requested: false,
      last_nmi: false,
      irq_requested: false,
      oam_dma_active: false,
      oam_dma_cycle: 0,
      oam_dma_address: 0,
      upcoming_write: false,
    }
  }
}



pub fn nmi_signal(nes: &NesState) -> bool {
    return ((nes.ppu.control & 0x80) & (nes.ppu.status & 0x80)) != 0;
}

pub fn irq_signal(nes: &NesState) -> bool {
  if nes.registers.flags.interrupts_disabled {
    return false;
  } else {
    return nes.apu.irq_signal() || nes.mapper.irq_flag();
  }
}

pub fn poll_for_interrupts(nes: &mut NesState) {
  let current_nmi = nmi_signal(&nes);
  let last_nmi = nes.registers.flags.last_nmi;
  nes.registers.flags.last_nmi = current_nmi;
  if current_nmi && !last_nmi {
    nes.cpu.nmi_requested = true;
  }
  nes.cpu.irq_requested = irq_signal(&nes);
}

pub fn interrupt_requested(nes: &NesState) -> bool {
  return nes.cpu.nmi_requested || nes.cpu.irq_requested;
}

pub fn halt_cpu(nes: &mut NesState) {
  // HALT the CPU. It died, jim.
  if nes.cpu.tick < 10 {
    println!("STP opcode encountered: {}", nes.cpu.opcode);
    println!("Proceeding to lock up CPU. Goodbye, cruel world!");
  }
  nes.cpu.tick = 10;
}

pub fn alu_block(nes: &mut NesState, addressing_mode_index: u8, opcode_index: u8) {
  let addressing_mode = match addressing_mode_index {
    // Zero Page Mode
    0b000 => &addressing::INDEXED_INDIRECT_X,
    0b001 => &addressing::ZERO_PAGE,
    0b010 => &addressing::IMMEDIATE,
    0b011 => &addressing::ABSOLUTE,
    0b100 => &addressing::INDIRECT_INDEXED_Y,
    0b101 => &addressing::ZERO_PAGE_INDEXED_X,
    0b110 => &addressing::ABSOLUTE_INDEXED_Y,
    0b111 => &addressing::ABSOLUTE_INDEXED_X,

    // Not implemented yet
    _ => &addressing::UNIMPLEMENTED,
  };

  match opcode_index {
    0b000 => {(addressing_mode.read)(nes, opcodes::ora)},
    0b001 => {(addressing_mode.read)(nes, opcodes::and)},
    0b010 => {(addressing_mode.read)(nes, opcodes::eor)},
    0b011 => {(addressing_mode.read)(nes, opcodes::adc)},
    0b100 => {(addressing_mode.write)(nes, opcodes::sta)},
    0b101 => {(addressing_mode.read)(nes, opcodes::lda)},
    0b110 => {(addressing_mode.read)(nes, opcodes::cmp)},
    0b111 => {(addressing_mode.read)(nes, opcodes::sbc)},
    _ => ()
  };
}

pub fn rmw_block(nes: &mut NesState, addressing_mode_index: u8, opcode_index: u8) {
  // First, handle some block 10 opcodes that break the mold
  match nes.cpu.opcode {
    // Assorted NOPs
    0x82 | 0xC2 | 0xE2 => (addressing::IMMEDIATE.read) (nes, opcodes::nop_read),
    0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xEA | 0xFA => addressing::implied(nes, opcodes::nop),
    // Certain opcodes may be vital to your success. THESE opcodes are not.
    0x02 | 0x22 | 0x42 | 0x62 | 0x12 | 0x32 | 0x52 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
      halt_cpu(nes);
    },
    0xA2 => {(addressing::IMMEDIATE.read)(nes, opcodes::ldx)},
    0x8A => addressing::implied(nes, opcodes::txa),
    0xAA => addressing::implied(nes, opcodes::tax),
    0xCA => addressing::implied(nes, opcodes::dex),
    0x9A => addressing::implied(nes, opcodes::txs),
    0xBA => addressing::implied(nes, opcodes::tsx),
    0x96 => {(addressing::ZERO_PAGE_INDEXED_Y.write)(nes, opcodes::stx)},
    0xB6 => {(addressing::ZERO_PAGE_INDEXED_Y.read)(nes, opcodes::ldx)},
    0xBE => {(addressing::ABSOLUTE_INDEXED_Y.read)(nes, opcodes::ldx)},
    0x9E => unofficial_opcodes::shx(nes),
    _ => {
      let addressing_mode = match addressing_mode_index {
        // Zero Page Mode
        0b001 => &addressing::ZERO_PAGE,
        0b010 => &addressing::ACCUMULATOR, // Note: masked for 8A, AA, CA, and EA above
        0b011 => &addressing::ABSOLUTE,
        0b101 => &addressing::ZERO_PAGE_INDEXED_X,
        0b111 => &addressing::ABSOLUTE_INDEXED_X,

        // Not implemented yet
        _ => &addressing::UNIMPLEMENTED,
      };

      match opcode_index {
        0b000 => {(addressing_mode.modify)(nes, opcodes::asl)},
        0b001 => {(addressing_mode.modify)(nes, opcodes::rol)},
        0b010 => {(addressing_mode.modify)(nes, opcodes::lsr)},
        0b011 => {(addressing_mode.modify)(nes, opcodes::ror)},
        0b100 => {(addressing_mode.write)(nes, opcodes::stx)},
        0b101 => {(addressing_mode.read)(nes, opcodes::ldx)},
        0b110 => {(addressing_mode.modify)(nes, opcodes::dec)},
        0b111 => {(addressing_mode.modify)(nes, opcodes::inc)},
        _ => ()
      };
    }
  };
}

pub fn control_block(nes: &mut NesState) {
  // Branch instructions are of the form xxy10000
  if (nes.cpu.opcode & 0b1_1111) == 0b1_0000 {
    return opcodes::branch(nes);
  }

  // Everything else is pretty irregular, so we'll just match the whole opcode
  match nes.cpu.opcode {
    0x00 => opcodes::brk(nes),

    // Various unofficial NOPs
    0x80 => 
      (addressing::IMMEDIATE.read)  (nes, opcodes::nop_read),
    0x04 | 0x44 | 0x64 => 
      (addressing::ZERO_PAGE.read)  (nes, opcodes::nop_read),
    0x0C => 
      (addressing::ABSOLUTE.read)  (nes, opcodes::nop_read),
    0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => 
      (addressing::ZERO_PAGE_INDEXED_X.read)  (nes, opcodes::nop_read),
    0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC =>
      (addressing::ABSOLUTE_INDEXED_X.read)  (nes, opcodes::nop_read),

    0x9C => unofficial_opcodes::shy(nes),

    // Opcodes with similar addressing modes
    0xA0 => (addressing::IMMEDIATE.read)  (nes, opcodes::ldy),
    0xC0 => (addressing::IMMEDIATE.read)  (nes, opcodes::cpy),
    0xE0 => (addressing::IMMEDIATE.read)  (nes, opcodes::cpx),
    0x24 => (addressing::ZERO_PAGE.read)  (nes, opcodes::bit),
    0x84 => (addressing::ZERO_PAGE.write) (nes, opcodes::sty),
    0xA4 => (addressing::ZERO_PAGE.read)  (nes, opcodes::ldy),
    0xC4 => (addressing::ZERO_PAGE.read)  (nes, opcodes::cpy),
    0xE4 => (addressing::ZERO_PAGE.read)  (nes, opcodes::cpx),
    0x2C => (addressing::ABSOLUTE.read)  (nes, opcodes::bit),
    0x8C => (addressing::ABSOLUTE.write) (nes, opcodes::sty),
    0xAC => (addressing::ABSOLUTE.read)  (nes, opcodes::ldy),
    0xCC => (addressing::ABSOLUTE.read)  (nes, opcodes::cpy),
    0xEC => (addressing::ABSOLUTE.read)  (nes, opcodes::cpx),
    0x94 => (addressing::ZERO_PAGE_INDEXED_X.write) (nes, opcodes::sty),
    0xB4 => (addressing::ZERO_PAGE_INDEXED_X.read)  (nes, opcodes::ldy),
    0xBC => (addressing::ABSOLUTE_INDEXED_X.read)  (nes, opcodes::ldy),

    0x4C => opcodes::jmp_absolute(nes),
    0x6C => opcodes::jmp_indirect(nes),

    0x08 => opcodes::php(nes),
    0x28 => opcodes::plp(nes),
    0x48 => opcodes::pha(nes),
    0x68 => opcodes::pla(nes),

    0x20 => opcodes::jsr(nes),
    0x40 => opcodes::rti(nes),
    0x60 => opcodes::rts(nes),

    0x88 => addressing::implied(nes, opcodes::dey),
    0xA8 => addressing::implied(nes, opcodes::tay),
    0xC8 => addressing::implied(nes, opcodes::iny),
    0xE8 => addressing::implied(nes, opcodes::inx),

    0x18 => addressing::implied(nes, opcodes::clc),
    0x58 => addressing::implied(nes, opcodes::cli),
    0xB8 => addressing::implied(nes, opcodes::clv),
    0xD8 => addressing::implied(nes, opcodes::cld),
    0x38 => addressing::implied(nes, opcodes::sec),
    0x78 => addressing::implied(nes, opcodes::sei),
    0xF8 => addressing::implied(nes, opcodes::sed),
    0x98 => addressing::implied(nes, opcodes::tya),

    _ => {
      // Unimplemented, fall back on old behavior
      println!("Undefined (0x00) opcode: {:02X}", nes.cpu.opcode);
      nes.cpu.tick = 0;
    }
  };
}

pub fn unofficial_block(nes: &mut NesState, addressing_mode_index: u8, opcode_index: u8) {
  // unofficial opcodes are surprisingly regular, but the following instructions break the
  // mold, mostly from the +0B block:
  match nes.cpu.opcode {
    0x0B | 0x2B => {(addressing::IMMEDIATE.read)(nes, unofficial_opcodes::anc)},
    0x4B => {(addressing::IMMEDIATE.read)(nes, unofficial_opcodes::alr)},
    0x6B => {(addressing::IMMEDIATE.read)(nes, unofficial_opcodes::arr)},
    0x8B => {(addressing::IMMEDIATE.read)(nes, unofficial_opcodes::xaa)},
    0x93 => unofficial_opcodes::ahx_indirect_indexed_y(nes),
    0x9B => unofficial_opcodes::tas(nes),
    0x97 => {(addressing::ZERO_PAGE_INDEXED_Y.write)(nes, unofficial_opcodes::sax)},
    0x9F => unofficial_opcodes::ahx_absolute_indexed_y(nes),
    0xB7 => {(addressing::ZERO_PAGE_INDEXED_Y.read)(nes, unofficial_opcodes::lax)},
    0xBB => {(addressing::ABSOLUTE_INDEXED_Y.read)(nes, unofficial_opcodes::las)},
    0xBF => {(addressing::ABSOLUTE_INDEXED_Y.read)(nes, unofficial_opcodes::lax)},
    0xCB => {(addressing::IMMEDIATE.read)(nes, unofficial_opcodes::axs)},
    0xEB => {(addressing::IMMEDIATE.read)(nes, opcodes::sbc)},
    _ => {
      // The remaining opcodes all use the same addressing mode as the ALU block, and the wame
      // read / write / modify type as the corresponding RMW block. Opcodes are mostly a combination
      // of the two, with a few exceptions.

      let addressing_mode = match addressing_mode_index {
        // Zero Page Mode
        0b000 => &addressing::INDEXED_INDIRECT_X,
        0b001 => &addressing::ZERO_PAGE,
        0b010 => &addressing::IMMEDIATE,
        0b011 => &addressing::ABSOLUTE,
        0b100 => &addressing::INDIRECT_INDEXED_Y,
        0b101 => &addressing::ZERO_PAGE_INDEXED_X,
        0b110 => &addressing::ABSOLUTE_INDEXED_Y,
        0b111 => &addressing::ABSOLUTE_INDEXED_X,

        // Not implemented yet
        _ => &addressing::UNIMPLEMENTED,
      };

      match opcode_index {
        0b000 => {(addressing_mode.modify)(nes, unofficial_opcodes::slo)},
        0b001 => {(addressing_mode.modify)(nes, unofficial_opcodes::rla)},
        0b010 => {(addressing_mode.modify)(nes, unofficial_opcodes::sre)},
        0b011 => {(addressing_mode.modify)(nes, unofficial_opcodes::rra)},
        0b100 => {(addressing_mode.write )(nes, unofficial_opcodes::sax)},
        0b101 => {(addressing_mode.read  )(nes, unofficial_opcodes::lax)},
        0b110 => {(addressing_mode.modify)(nes, unofficial_opcodes::dcp)},
        0b111 => {(addressing_mode.modify)(nes, unofficial_opcodes::isc)},
        _ => ()
      };
    }
  }
}

pub fn advance_oam_dma(nes: &mut NesState) {
  if nes.cpu.oam_dma_cycle & 0b1 == 0 && nes.cpu.oam_dma_cycle <= 511 {
    let address = nes.cpu.oam_dma_address;
    let oam_byte = read_byte(nes, address);
    write_byte(nes, 0x2004, oam_byte);
    nes.cpu.oam_dma_address += 1;
  }
  
  if nes.cpu.oam_dma_cycle & 0b1 == 0 || nes.apu.dmc.rdy_line == false {
    nes.cpu.oam_dma_cycle += 1;
  }  

  if nes.cpu.oam_dma_cycle > 513 {
    nes.cpu.oam_dma_active = false;
  }
}

pub fn run_one_clock(nes: &mut NesState) {
  if nes.cpu.oam_dma_active {
    advance_oam_dma(nes);
    return;
  }

  if nes.cpu.upcoming_write == false && nes.apu.dmc.rdy_line == true {
    // The DMC DMA is active during an upcoming READ cycle. PAUSE until the rdy_line
    // is no longer being asserted by the APU.
    return;
  }

  nes.cpu.tick += 1;

  // The ordering of these checks may seem a bit strange. The 6502 polls for interrupts
  // at the START of each cycle, not at the end. This means that whether an interrupt is
  // serviced is determined right before the last cycle of a given instruction, not after
  // the last cycle as one might expect.

  if nes.cpu.tick == 1 && interrupt_requested(&nes) {
    nes.cpu.service_routine_active = true;
  }

  poll_for_interrupts(nes);

  if nes.cpu.service_routine_active {
    return opcodes::service_interrupt(nes);
  }

  // Universal behavior for every opcode
  if nes.cpu.tick == 1 {
    // Fetch opcode from memory
    let pc = nes.registers.pc;
    nes.cpu.opcode = read_byte(nes, pc);
    nes.registers.pc = nes.registers.pc.wrapping_add(1);
    return; // all done
  }

  // Decode this opcode into its component parts
  let logic_block = nes.cpu.opcode & 0b0000_0011;
  let addressing_mode_index = (nes.cpu.opcode & 0b0001_1100) >> 2;
  let opcode_index = (nes.cpu.opcode & 0b1110_0000) >> 5;

  match logic_block {
    0b00 => control_block(nes),
    0b01 => alu_block(nes, addressing_mode_index, opcode_index),
    0b10 => rmw_block(nes, addressing_mode_index, opcode_index),
    0b11 => unofficial_block(nes, addressing_mode_index, opcode_index),
    _ => ()
  }
}