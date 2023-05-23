use addressing;
use cycle_cpu::Registers;
use opcodes;
use nes::NesState;
use memory::read_byte;
use memory::write_byte;

// Note: Opcode names follow the undefined opcodes tabke here:
// https://wiki.nesdev.com/w/index.php/CPU_unofficial_opcodes

// Shift left and inclusive OR A
pub fn slo(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::asl(registers, data);
  opcodes::ora(registers, result);
  return result;
}

// Rotate left, then AND A
pub fn rla(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::rol(registers, data);
  opcodes::and(registers, result);
  return result;
}

// Shift right, then Exclisive OR A
pub fn sre(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::lsr(registers, data);
  opcodes::eor(registers, result);
  return result;
}

// Rotate right, then ADC result with A
pub fn rra(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::ror(registers, data);
  opcodes::adc(registers, result);
  return result;
}

pub fn sax(registers: &mut Registers) -> u8 {
  let result = registers.a & registers.x;
  return result;
}

pub fn lax(registers: &mut Registers, data: u8) {
  opcodes::lda(registers, data);
  registers.x = data;
}

// Decrement and compare
pub fn dcp(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::dec(registers, data);
  opcodes::cmp(registers, result);
  return result;
}

// Increment and subtract w/ carry
pub fn isc(registers: &mut Registers, data: u8) -> u8 {
  let result = opcodes::inc(registers, data);
  opcodes::sbc(registers, result);
  return result;
}

// Many of the following opcodes are unstable, and therefore not part of official tests.
// Hardware results may depend on the alignment of the planets, and whether or not the code 
// is being run on an odd-numbered thursday in a month that ends with R.

// The following opcodes perform an &H, which requires the address byte to be available at a certain
// point. These are weird enough to break the opcode structure, and so get custom functions.

// UNSTABLE. Performs A & X & H, where H is usually the high byte of the target address + 1.
pub fn ahx_indirect_indexed_y(nes: &mut NesState) {
  match nes.cpu.tick {
    2 => addressing::read_data1(nes),
    3 => {
      // Read low byte of indirect address
      let pointer_low = nes.cpu.data1 as u16;
      nes.cpu.temp_address = read_byte(nes, pointer_low) as u16;
      nes.cpu.data1 = nes.cpu.data1.wrapping_add(1);
    },
    4 => {
      // Read high byte of indirect address
      let pointer_high = nes.cpu.data1 as u16;
      nes.cpu.temp_address = ((read_byte(nes, pointer_high) as u16) << 8) | nes.cpu.temp_address;
    },
    5 => {
      // Add Y to LOW BYTE of the effective address
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      // Save the result of our weird modification here:
      nes.cpu.data1 = nes.registers.a & nes.registers.x & (nes.cpu.temp_address.wrapping_add(0x100) >> 8) as u8;
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        //nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
        nes.cpu.temp_address = (nes.cpu.temp_address & 0x00FF) | ((nes.cpu.data1 as u16) << 8);
      }
    },
    6 => {
      let data = nes.cpu.data1;
      let address = nes.cpu.temp_address;
      write_byte(nes, address, data);
      nes.cpu.tick = 0;
    },
    _ => {}
  }
}

pub fn ahx_absolute_indexed_y(nes: &mut NesState) {
  match nes.cpu.tick {
    2 => addressing::read_address_low(nes),
    3 => addressing::read_address_high(nes),
    4 => {
      // Add Y to LOW BYTE of the effective address
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      // Save the result of our weird modification here:
      nes.cpu.data1 = nes.registers.a & nes.registers.x & (nes.cpu.temp_address.wrapping_add(0x100) >> 8) as u8;
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = (nes.cpu.temp_address & 0x00FF) | ((nes.cpu.data1 as u16) << 8);
      }
    },
    5 => {
      let data = nes.cpu.data1;
      let address = nes.cpu.temp_address;
      write_byte(nes, address, data);
      nes.cpu.tick = 0;
    },
    _ => {}
  }
}

pub fn tas(nes: &mut NesState) {
  match nes.cpu.tick {
    2 => addressing::read_address_low(nes),
    3 => addressing::read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);

      // First, set S = A & X
      nes.registers.s = nes.registers.a & nes.registers.x;

      // Now, AND that result with the high byte + 1, and save it for later writing
      nes.cpu.data1 = nes.registers.s & (nes.cpu.temp_address.wrapping_add(0x100) >> 8) as u8;

      if low_byte > 0xFF {
        // Use our calculated value as the high byte, effectively corrupting the page boundary
        // crossing logic. Where will we write? It's a mystery!
        nes.cpu.temp_address = (nes.cpu.temp_address & 0x00FF) | ((nes.cpu.data1 as u16) << 8);
      }
    },
    5 => {
      let data = nes.cpu.data1;
      let address = nes.cpu.temp_address;
      write_byte(nes, address, data);
      nes.cpu.tick = 0;
    },
    _ => {}
  }
}

// Increment and subtract w/ carry
pub fn las(registers: &mut Registers, data: u8) {
  let result = registers.s & data;
  registers.a = result;
  registers.x = result;
  registers.s = result;
}

// AND with carry
pub fn anc(registers: &mut Registers, data: u8) {
  opcodes::and(registers, data);
  registers.flags.carry = (registers.a & 0b1000_0000) != 0;
}

// AND with #imm, then LSR
pub fn alr(registers: &mut Registers, data: u8) {
  opcodes::and(registers, data);
  let result = registers.a;
  registers.a = opcodes::lsr(registers, result);
}

// AND with #imm, then ROR
pub fn arr(registers: &mut Registers, data: u8) {
  opcodes::and(registers, data);
  let result = registers.a;
  registers.a = opcodes::ror(registers, result);
  // Carry and Overflow are set weirdly:
  registers.flags.carry = (registers.a & 0b0100_0000) != 0;
  registers.flags.overflow = (((registers.a & 0b0100_0000) >> 6) ^ ((registers.a & 0b0010_0000) >> 5)) != 0;
}

// "Magic" value is a complete guess. I don't know if the NES's decimal unit actually
// exists and is stubbed out; I'm assuming here that it is NOT, so setting magic to
// 0x00. The true effect of this instruction varies *by console* and the instruction
// should not be used for any purpose.
// http://visual6502.org/wiki/index.php?title=6502_Opcode_8B_%28XAA,_ANE%29
pub fn xaa(registers: &mut Registers, data: u8) {
  // A = (A | magic) & X & imm
  let magic = 0x00;
  registers.a = (registers.a | magic) & registers.x & data;
  registers.flags.zero = registers.a == 0;
  registers.flags.negative = registers.a & 0x80 != 0;
}

pub fn axs(registers: &mut Registers, data: u8) {
  let initial = registers.a & registers.x;
  // CMP with #imm, but store value in x:
  registers.flags.carry = initial >= data;
  registers.x = initial.wrapping_sub(data);
  registers.flags.zero = registers.x == 0;
  registers.flags.negative = registers.x & 0x80 != 0;
}

pub fn shx(nes: &mut NesState) {
  match nes.cpu.tick {
    2 => addressing::read_address_low(nes),
    3 => addressing::read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      // Save the result of our weird modification here:
      nes.cpu.data1 = nes.registers.x & (nes.cpu.temp_address.wrapping_add(0x100) >> 8) as u8;
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = (nes.cpu.temp_address & 0x00FF) | ((nes.cpu.data1 as u16) << 8);
      }
    },
    5 => {
      let data = nes.cpu.data1;
      let address = nes.cpu.temp_address;
      write_byte(nes, address, data);
      nes.cpu.tick = 0;
    },
    _ => {}
  }
}

pub fn shy(nes: &mut NesState) {
  match nes.cpu.tick {
    2 => addressing::read_address_low(nes),
    3 => addressing::read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.x as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      // Save the result of our weird modification here:
      nes.cpu.data1 = nes.registers.y & (nes.cpu.temp_address.wrapping_add(0x100) >> 8) as u8;
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = (nes.cpu.temp_address & 0x00FF) | ((nes.cpu.data1 as u16) << 8);
      }
    },
    5 => {
      let data = nes.cpu.data1;
      let address = nes.cpu.temp_address;
      write_byte(nes, address, data);
      nes.cpu.tick = 0;
    },
    _ => {}
  }
}