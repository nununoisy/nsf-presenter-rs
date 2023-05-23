use cycle_cpu::Registers;
use memory::read_byte;
use memory::write_byte;
use nes::NesState;

type ImpliedOpcode = fn(&mut Registers);
type ReadOpcode = fn(&mut Registers, u8);
type WriteOpcode = fn(&mut Registers) -> u8;
type ModifyOpcode = fn(&mut Registers, u8) -> u8;

pub struct AddressingMode {
  pub read: fn(&mut NesState, ReadOpcode),
  pub write: fn(&mut NesState, WriteOpcode),
  pub modify: fn(&mut NesState, ModifyOpcode),
}

// Common helper functions used on many instruction cycles
pub fn read_data1(nes: &mut NesState) {
  let pc = nes.registers.pc;
  nes.cpu.data1 = read_byte(nes, pc);
  nes.registers.pc = nes.registers.pc.wrapping_add(1);
}

// Every instruction reads the byte after PC, but not all instructions use the
// data. This is the memory access for those instructions which don't.
pub fn dummy_data1(nes: &mut NesState) {
  let pc = nes.registers.pc;
  let _ = read_byte(nes, pc);
}

pub fn read_address_low(nes: &mut NesState) {
  // Just read data1 here, we'll combine when reading the high byte
  read_data1(nes);
}

pub fn read_address_high(nes: &mut NesState) {
  let pc = nes.registers.pc;
  nes.cpu.data2 = read_byte(nes, pc);
  nes.registers.pc = nes.registers.pc.wrapping_add(1);
  nes.cpu.temp_address = ((nes.cpu.data2 as u16) << 8) | (nes.cpu.data1 as u16);
}

fn read_opcode_from_temp_address(nes: &mut NesState, opcode_func: ReadOpcode, last_cycle: bool) {
  let temp_address = nes.cpu.temp_address;
  let data = read_byte(nes, temp_address);
  opcode_func(&mut nes.registers, data);
  if last_cycle { 
    nes.cpu.tick = 0;
  }
}

fn write_opcode_to_temp_address(nes: &mut NesState, opcode_func: WriteOpcode, last_cycle: bool) {
  // Write the result of this opcode to the final address
  let temp_address = nes.cpu.temp_address;
  let data = opcode_func(&mut nes.registers);
  write_byte(nes, temp_address, data);
  if last_cycle { 
    nes.cpu.tick = 0;
  }
}

// Simplest possible opcode, immediately changes register state and
// exits on cycle 2
pub fn implied(nes: &mut NesState, opcode_func: ImpliedOpcode) {
  opcode_func(&mut nes.registers);
  nes.cpu.tick = 0;
}

// Note: These should never be called. They exist to make some of the decoding logic easier
// in cycle_cpu.rs. Any erroneous calls are therefore fatal.
pub fn unimplemented_read(nes: &mut NesState, _: ReadOpcode) {
  panic!("unimplemented read {:02X}", nes.cpu.opcode);
}

pub fn unimplemented_write(nes: &mut NesState, _: WriteOpcode) {
  panic!("unimplemented write {:02X}", nes.cpu.opcode);
}

pub fn unimplemented_modify(nes: &mut NesState, _: ModifyOpcode) {
  panic!("unimplemented modify {:02X}", nes.cpu.opcode);
}

// Called by STA in #imm mode, this has the effect of a two-byte NOP
// which skips the data byte. but still takes just 2 cycles.
pub fn nop_write(nes: &mut NesState, _: WriteOpcode) {
  nes.registers.pc = nes.registers.pc.wrapping_add(1);
  nes.cpu.tick = 0;
}

pub static UNIMPLEMENTED: AddressingMode = AddressingMode{
  read: unimplemented_read, write: unimplemented_write, modify: unimplemented_modify };

// Accumulator mode is used by instructions which modify the accumulator without reading memory.
pub fn accumulator_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  let a = nes.registers.a;
  let result = opcode_func(&mut nes.registers, a);
  nes.registers.a = result;
  nes.cpu.tick = 0;
}

pub static ACCUMULATOR: AddressingMode = AddressingMode{
  read: unimplemented_read, write: unimplemented_write, modify: accumulator_modify };

// Immediate mode only supports reading data
pub fn immediate_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  let pc = nes.registers.pc;
  let data = read_byte(nes, pc);
  opcode_func(&mut nes.registers, data);
  nes.registers.pc = nes.registers.pc.wrapping_add(1);
  nes.cpu.tick = 0;
}

pub static IMMEDIATE: AddressingMode = AddressingMode{
  read: immediate_read, write: nop_write, modify: unimplemented_modify };

// Absolute mode
pub fn absolute_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      let effective_address = nes.cpu.temp_address;
      let data = read_byte(nes, effective_address);
      opcode_func(&mut nes.registers, data);
      nes.cpu.tick = 0;
    },
    _ => ()
  }
}

pub fn absolute_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => {
      read_address_high(nes);
      nes.cpu.upcoming_write = true;
    },
    4 => {
      let effective_address = nes.cpu.temp_address;
      let data = opcode_func(&mut nes.registers);
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub fn absolute_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      let effective_address = nes.cpu.temp_address;
      nes.cpu.data1 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    5 => {
      // Dummy write the original value back to the effective address
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      // Run the opcode on the data
      nes.cpu.data1 = opcode_func(&mut nes.registers, data);
    },
    6 => {
      // Write the modified data back out to the effective address
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      // All done
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub static ABSOLUTE: AddressingMode = AddressingMode{
  read: absolute_read, write: absolute_write, modify: absolute_modify };

// Zero Page mode
pub fn zero_page_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = read_byte(nes, effective_address);
      opcode_func(&mut nes.registers, data);
      nes.cpu.tick = 0;
    },
    _ => ()
  }
}

pub fn zero_page_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => {
      read_data1(nes);
      nes.cpu.upcoming_write = true;
    },
    3 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = opcode_func(&mut nes.registers);
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub fn zero_page_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.data2 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    4 => {
      // Dummy write to the zero-page address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);

      // Perform the opcode on the data
      let result = opcode_func(&mut nes.registers, data);
      nes.cpu.data2 = result;
    },
    5 => {
      // Write the result to the effective address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub static ZERO_PAGE: AddressingMode = AddressingMode{
  read: zero_page_read, write: zero_page_write, modify: zero_page_modify };

pub fn add_to_zero_page_address(nes: &mut NesState, offset: u8) {
  let effective_address = nes.cpu.data1 as u16;
  // Dummy read from original address, discarded
  let _ = read_byte(nes, effective_address);
  nes.cpu.data1 = nes.cpu.data1.wrapping_add(offset);
}

// Zero Page Indexed (X)
pub fn zero_page_indexed_x_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.x; add_to_zero_page_address(nes, offset)},
    4 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = read_byte(nes, effective_address);
      opcode_func(&mut nes.registers, data);
      nes.cpu.tick = 0;
    },
    _ => ()
  }
}

pub fn zero_page_indexed_x_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {
      let offset = nes.registers.x; 
      add_to_zero_page_address(nes, offset);
      nes.cpu.upcoming_write = true;
    },
    4 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = opcode_func(&mut nes.registers);
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub fn zero_page_indexed_x_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.x; add_to_zero_page_address(nes, offset)},
    4 => {
      // Read the value at our effective address for processing
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.data2 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    5 => {
      // Dummy write back to effective address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);
      // Perform the operation
      nes.cpu.data2 = opcode_func(&mut nes.registers, data);
    },
    6 => {
      // Write the modified data to the effective address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub static ZERO_PAGE_INDEXED_X: AddressingMode = AddressingMode{
  read: zero_page_indexed_x_read, write: zero_page_indexed_x_write, modify: zero_page_indexed_x_modify };

// Zero Page Indexed (Y)
pub fn zero_page_indexed_y_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.y; add_to_zero_page_address(nes, offset)},
    4 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = read_byte(nes, effective_address);
      opcode_func(&mut nes.registers, data);
      nes.cpu.tick = 0;
    },
    _ => ()
  }
}

pub fn zero_page_indexed_y_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {
      let offset = nes.registers.y; 
      add_to_zero_page_address(nes, offset);
      nes.cpu.upcoming_write = true;
    },
    4 => {
      let effective_address = nes.cpu.data1 as u16;
      let data = opcode_func(&mut nes.registers);
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub fn zero_page_indexed_y_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.y; add_to_zero_page_address(nes, offset)},
    4 => {
      // Read the value at our effective address for processing
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.data2 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    5 => {
      // Dummy write back to effective address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);
      // Perform the operation
      nes.cpu.data2 = opcode_func(&mut nes.registers, data);
    },
    6 => {
      // Write the modified data to the effective address
      let effective_address = nes.cpu.data1 as u16;
      let data = nes.cpu.data2;
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub static ZERO_PAGE_INDEXED_Y: AddressingMode = AddressingMode{
  read: zero_page_indexed_y_read, write: zero_page_indexed_y_write, modify: zero_page_indexed_y_modify };

pub fn indexed_indirect_x_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.x; add_to_zero_page_address(nes, offset)},
    4 => {
      // Read low byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = read_byte(nes, effective_address) as u16;
      nes.cpu.data1 = nes.cpu.data1.wrapping_add(1);
    },
    5 => {
      // Read high byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = ((read_byte(nes, effective_address) as u16) << 8) | nes.cpu.temp_address;
    },
    6 => read_opcode_from_temp_address(nes, opcode_func, true),
    _ => {}
  }
}

pub fn indexed_indirect_x_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.x; add_to_zero_page_address(nes, offset)},
    4 => {
      // Read low byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = read_byte(nes, effective_address) as u16;
      nes.cpu.data1 = nes.cpu.data1.wrapping_add(1);
    },
    5 => {
      // Read high byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = ((read_byte(nes, effective_address) as u16) << 8) | nes.cpu.temp_address;
      nes.cpu.upcoming_write = true;
    },
    6 => {
      write_opcode_to_temp_address(nes, opcode_func, true);
      nes.cpu.upcoming_write = false;
    },
    _ => {}
  }
}

pub fn indexed_indirect_x_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
    3 => {let offset = nes.registers.x; add_to_zero_page_address(nes, offset)},
    4 => {
      // Read low byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = read_byte(nes, effective_address) as u16;
      nes.cpu.data1 = nes.cpu.data1.wrapping_add(1);
    },
    5 => {
      // Read high byte of indirect address
      let effective_address = nes.cpu.data1 as u16;
      nes.cpu.temp_address = ((read_byte(nes, effective_address) as u16) << 8) | nes.cpu.temp_address;
    },
    6 => {
      // Read value from effective address
      let effective_address = nes.cpu.temp_address;
      nes.cpu.data1 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    7 => {
      // Dummy write of the original value
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      // Perform opcode and store
      nes.cpu.data1 = opcode_func(&mut nes.registers, data);
    },
    8 => {
      // Finally write modified data back out to effective_address
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }
}

pub static INDEXED_INDIRECT_X: AddressingMode = AddressingMode{
  read: indexed_indirect_x_read, write: indexed_indirect_x_write, modify: indexed_indirect_x_modify};

pub fn indirect_indexed_y_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
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
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      // Read from this new address
      let temp_address = nes.cpu.temp_address;
      let data = read_byte(nes, temp_address);
      // If the new address doesn't need adjustment, run the opcode now and bail early, intentionally
      // skipping cycle 6
      if low_byte <= 0xFF {
        opcode_func(&mut nes.registers, data);
        nes.cpu.tick = 0;
      } else {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);  
      }
    },
    6 => read_opcode_from_temp_address(nes, opcode_func, true),
    _ => {}
  }
}

pub fn indirect_indexed_y_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
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
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
      nes.cpu.upcoming_write = true;
    },
    6 => {
      write_opcode_to_temp_address(nes, opcode_func, true);
      nes.cpu.upcoming_write = false;
    },
    _ => {}
  }
}

pub fn indirect_indexed_y_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {
  match nes.cpu.tick {
    2 => read_data1(nes),
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
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5 for simplicity
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
    },
    6 => {
      // Read value from effective address
      let effective_address = nes.cpu.temp_address;
      nes.cpu.data1 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    7 => {
      // Dummy write of the original value
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      // Perform opcode and store
      nes.cpu.data1 = opcode_func(&mut nes.registers, data);
    },
    8 => {
      // Finally write modified data back out to effective_address
      let effective_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, effective_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    },
    _ => ()
  }

}

pub static INDIRECT_INDEXED_Y: AddressingMode = AddressingMode{
  read: indirect_indexed_y_read, write: indirect_indexed_y_write, modify: indirect_indexed_y_modify };

pub fn absolute_indexed_x_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 3
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.x as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      // Read from this new address
      let temp_address = nes.cpu.temp_address;
      let data = read_byte(nes, temp_address);
      // If the new address doesn't need adjustment, run the opcode now and bail early, intentionally
      // skipping cycle 5
      if low_byte <= 0xFF {
        opcode_func(&mut nes.registers, data);
        nes.cpu.tick = 0;
      } else {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);  
      }
    },
    5 => read_opcode_from_temp_address(nes, opcode_func, true),
    _ => ()
  }
}

pub fn absolute_indexed_x_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.x as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      // Dummy read from the new address before it is fixed
      let temp_address = nes.cpu.temp_address;
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
      nes.cpu.upcoming_write = true;
    },
    5 => {
      write_opcode_to_temp_address(nes, opcode_func, true);
      nes.cpu.upcoming_write = false;
    },
    _ => {}
  }
}

pub fn absolute_indexed_x_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {  
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add X to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.x as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
    },
    5 => {
      let effective_address = nes.cpu.temp_address;
      nes.cpu.data1 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    6 => {
      // Dummy write
      let temp_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, temp_address, data);
      // Perform opcode and store
      nes.cpu.data1 = opcode_func(&mut nes.registers, data);
    },
    7 => {
      // Finally write modified data back out to effective_address
      let temp_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, temp_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    }
    _ => (),
  }
}

pub static ABSOLUTE_INDEXED_X: AddressingMode = AddressingMode{
  read: absolute_indexed_x_read, write: absolute_indexed_x_write, modify: absolute_indexed_x_modify };

pub fn absolute_indexed_y_read(nes: &mut NesState, opcode_func: ReadOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add Y to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 3
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      // Read from this new address
      let temp_address = nes.cpu.temp_address;
      let data = read_byte(nes, temp_address);
      // If the new address doesn't need adjustment, run the opcode now and bail early, intentionally
      // skipping cycle 5
      if low_byte <= 0xFF {
        opcode_func(&mut nes.registers, data);
        nes.cpu.tick = 0;
      } else {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);  
      }
    },
    5 => read_opcode_from_temp_address(nes, opcode_func, true),
    _ => ()
  }
}

pub fn absolute_indexed_y_write(nes: &mut NesState, opcode_func: WriteOpcode) {
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add Y to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
      nes.cpu.upcoming_write = true;
    },
    5 => {
      write_opcode_to_temp_address(nes, opcode_func, true);
      nes.cpu.upcoming_write = false;
    },
    _ => {}
  }
}

pub fn absolute_indexed_y_modify(nes: &mut NesState, opcode_func: ModifyOpcode) {  
  match nes.cpu.tick {
    2 => read_address_low(nes),
    3 => read_address_high(nes),
    4 => {
      // Add Y to LOW BYTE of the effective address
      // Accuracy note: technically this occurs in cycle 4, but as it has no effect on emulation, I've
      // moved it to the beginning of cycle 5, as it makes the early escape detection more straightforward.
      let low_byte = (nes.cpu.temp_address & 0xFF) + (nes.registers.y as u16);
      nes.cpu.temp_address = (nes.cpu.temp_address & 0xFF00) | (low_byte & 0xFF);
      let temp_address = nes.cpu.temp_address;
      // Dummy read from the new address before it is fixed
      let _ = read_byte(nes, temp_address);
      if low_byte > 0xFF {
        // Fix the high byte of the address by adding 1 to it
        nes.cpu.temp_address = nes.cpu.temp_address.wrapping_add(0x100);
      }
    },
    5 => {
      let effective_address = nes.cpu.temp_address;
      nes.cpu.data1 = read_byte(nes, effective_address);
      nes.cpu.upcoming_write = true;
    },
    6 => {
      // Dummy write
      let temp_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, temp_address, data);
      // Perform opcode and store
      nes.cpu.data1 = opcode_func(&mut nes.registers, data);
    },
    7 => {
      // Finally write modified data back out to effective_address
      let temp_address = nes.cpu.temp_address;
      let data = nes.cpu.data1;
      write_byte(nes, temp_address, data);
      nes.cpu.tick = 0;
      nes.cpu.upcoming_write = false;
    }
    _ => (),
  }
}

pub static ABSOLUTE_INDEXED_Y: AddressingMode = AddressingMode{
  read: absolute_indexed_y_read, write: absolute_indexed_y_write, modify: absolute_indexed_y_modify };

