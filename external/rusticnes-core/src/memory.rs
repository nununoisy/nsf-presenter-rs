use nes::NesState;

pub struct CpuMemory {
    pub iram_raw: Vec<u8>,

    pub recent_reads: Vec<u16>,
    pub recent_writes: Vec<u16>,
    pub open_bus: u8
}

impl CpuMemory {
    pub fn new() -> CpuMemory {
        return CpuMemory {
            iram_raw: vec!(0u8; 0x800),
            recent_reads: Vec::new(),
            recent_writes: Vec::new(),
            open_bus: 0,
        }
    }
}

pub fn debug_read_byte(nes: &NesState, address: u16) -> u8 {
    // Handle a few special cases for debug reads
    match address {
        0x2000 ..= 0x3FFF => {
            let ppu_reg = address & 0x7;
            match ppu_reg {
                7 => {
                    let ppu_addr = nes.ppu.current_vram_address;
                    // Note: does not simulate the data / palette fetch quirk.
                    return nes.ppu.debug_read_byte(& *nes.mapper, ppu_addr);
                },
                _ => {}
            }
        },
        0x4015 => {
            return nes.apu.debug_read_register(address);
        },
        _ => {}
    }

    let mapped_byte = nes.mapper.debug_read_cpu(address).unwrap_or(nes.memory.open_bus);
    return _read_byte(nes, address, mapped_byte);
}

pub fn read_byte(nes: &mut NesState, address: u16) -> u8 {
    let mapped_byte = nes.mapper.read_cpu(address).unwrap_or(nes.memory.open_bus);

    // This is a live read, handle any side effects
    match address {
        0x2000 ..= 0x3FFF => {
            let ppu_reg = address & 0x7;
            match ppu_reg {
                // PPUSTATUS
                2 => {
                    nes.ppu.write_toggle = false;
                    nes.ppu.latch = (nes.ppu.status & 0xE0) + (nes.ppu.latch & 0x1F);
                    nes.ppu.status = nes.ppu.status & 0x7F; // Clear VBlank bit
                    nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, nes.ppu.latch);
                    return nes.ppu.latch;
                },
                // OAMDATA
                4 => {
                    nes.ppu.latch = nes.ppu.oam[nes.ppu.oam_addr as usize];
                    nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, nes.ppu.latch);
                },
                // PPUDATA
                7 => {
                    let ppu_addr = nes.ppu.current_vram_address;
                    nes.ppu.latch = nes.ppu.read_latched_byte(&mut *nes.mapper, ppu_addr);
                    if nes.ppu.rendering_enabled() && 
                    (nes.ppu.current_scanline == 261 ||
                     nes.ppu.current_scanline <= 239) {
                        // Glitchy increment, a fine y and a coarse x 
                        nes.ppu.increment_coarse_x();
                        nes.ppu.increment_fine_y();
                    } else {
                        // Normal incrementing behavior based on PPUCTRL
                        if nes.ppu.control & 0x04 == 0 {
                            nes.ppu.current_vram_address += 1;
                        } else {
                            nes.ppu.current_vram_address += 32;
                        }
                        nes.ppu.current_vram_address &= 0b0111_1111_1111_1111;
                    }
                    // Perform a dummy access immediately, to simulte the behavior of the PPU
                    // address lines changing, so the mapper can react accordingly
                    let address = nes.ppu.current_vram_address;
                    nes.mapper.access_ppu(address);
                    nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, nes.ppu.latch);
                },
                _ => {}
            }
        },
        0x4015 => {
            let apu_byte = nes.apu.read_register(address);
            nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, apu_byte);
            return apu_byte;
        },
        0x4016 => {
            if nes.input_latch {
                // strobe register is high, so copy input data to latch (probably bad if this
                // actually occurs here, but it matches what real hardware would do)
                nes.p1_data = nes.p1_input;
            }
            let result = 0x40 | (nes.p1_data & 0x1);
            // Standard Controllers set extra bits to 1, which affects controller detection routines
            nes.p1_data = (nes.p1_data >> 1) | 0x80; 
            nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, result);
            return result;
        },
        0x4017 => {
            if nes.input_latch {
                // strobe register is high, so copy input data to latch (probably bad if this
                // actually occurs here, but it matches what real hardware would do)
                nes.p2_data = nes.p2_input;
            }
            let result = 0x40 | (nes.p2_data & 0x1);
            // Standard Controllers set extra bits to 1, which affects controller detection routines
            nes.p2_data = (nes.p2_data >> 1) | 0x80; 
            nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, result);
            return result;
        },
        _ => {}
    }

    let byte = _read_byte(nes, address, mapped_byte);
    nes.memory.open_bus = byte;
    nes.event_tracker.snoop_cpu_read(nes.registers.pc, address, byte);
    return byte;
}

fn _read_byte(nes: &NesState, address: u16, mapped_byte: u8) -> u8 {
    match address {
        0x0000 ..= 0x1FFF => {
            return nes.memory.iram_raw[(address & 0x7FF) as usize];
        },
        0x2000 ..= 0x3FFF => {
            // PPU
            let ppu_reg = address & 0x7;
            match ppu_reg {
                // PPUCTRL, PPUMASK, OAMADDR | PPUSCROLL | PPUADDR (Write Only)
                0 | 1 | 3 | 5 | 6 => {
                    return nes.ppu.latch;
                },
                // PPUSTATUS
                2 => {
                    return (nes.ppu.status & 0xE0) + (nes.ppu.latch & 0x1F);
                },
                // OAMDATA
                4 => {
                    return nes.ppu.oam[nes.ppu.oam_addr as usize];
                },
                // PPUDATA
                7 => {
                    return nes.ppu.latch;
                },
                _ => return 0
            }
        },
        0x4016 => {
            let result = 0x40 | (nes.p1_data & 0x1);
            return result;
        },
        0x4017 => {
            let result = 0x40 | (nes.p2_data & 0x1);
            return result;
        },
        0x4020 ..= 0xFFFF => {
            return mapped_byte;
        },
        _ => {
            return nes.memory.open_bus;
        }
    }
}

pub fn write_byte(nes: &mut NesState, address: u16, data: u8) {
    // Track every byte written, unconditionally
    // (filtering is done inside the tracker)
    nes.event_tracker.snoop_cpu_write(nes.registers.pc, address, data);

    // The mapper *always* sees the write. Even to RAM, and even to internal registers.
    // Most mappers ignore writes to addresses below 0x6000. Some (notably MMC5) do not.
    nes.mapper.write_cpu(address, data);
    match address {
        0x0000 ..= 0x1FFF => nes.memory.iram_raw[(address & 0x7FF) as usize] = data,
        0x2000 ..= 0x3FFF => {
            // PPU
            let ppu_reg = address & 0x7;
            nes.ppu.latch = data;
            match ppu_reg {
                // PPUCTRL
                0 => {
                    nes.ppu.control = data;
                    // Shift the nametable select bits into the temporary vram address
                    //                                  yyy_nn_YYYYY_XXXXX
                    nes.ppu.temporary_vram_address &= 0b111_00_11111_11111;
                    nes.ppu.temporary_vram_address |= (data as u16 & 0b11) << 10;
                },
                // PPU MASK
                1 => {
                    nes.ppu.mask = data;
                },
                // PPUSTATUS is read-only
                // OAM ADDRESS
                3 => {
                    nes.ppu.oam_addr = data;
                },
                // OAMDATA
                4 => {
                    nes.ppu.oam[nes.ppu.oam_addr as usize] = data;
                    nes.ppu.oam_addr = nes.ppu.oam_addr.wrapping_add(1);
                },
                // PPU SCROLL
                5 => {
                    if nes.ppu.write_toggle {
                        // Set coarse Y and fine y into temporary address
                        //                                  yyy_nn_YYYYY_XXXXX
                        nes.ppu.temporary_vram_address &= 0b000_11_00000_11111;
                        nes.ppu.temporary_vram_address |= ((data as u16) & 0b1111_1000) << 2;
                        nes.ppu.temporary_vram_address |= ((data as u16) & 0b111) << 12;

                        nes.ppu.write_toggle = false;
                    } else {
                        // Set coarse X into temporary address
                        //                                  yyy_nn_YYYYY_XXXXX
                        nes.ppu.temporary_vram_address &= 0b111_11_11111_00000;
                        nes.ppu.temporary_vram_address |= (data as u16) >> 3;
                        // Set fine X immediately
                        nes.ppu.fine_x = data & 0b111;

                        nes.ppu.write_toggle = true;
                    }
                },
                // PPU ADDR
                6 => {
                    if nes.ppu.write_toggle {
                        nes.ppu.temporary_vram_address &= 0b0111_1111_0000_0000;
                        nes.ppu.temporary_vram_address |= data as u16;
                        // Apply the final vram address immediately
                        nes.ppu.current_vram_address = nes.ppu.temporary_vram_address;
                        nes.ppu.write_toggle = false;
                        
                        // Perform a dummy access immediately, to simulte the behavior of the PPU
                        // address lines changing, so the mapper can react accordingly
                        let address = nes.ppu.current_vram_address;
                        nes.mapper.access_ppu(address);
                    } else {
                        nes.ppu.temporary_vram_address &= 0b0000_0000_1111_1111;
                        // Note: This is missing bit 14 on purpose! This is cleared by the real PPU during
                        // the write to PPU ADDR for reasons unknown.
                        nes.ppu.temporary_vram_address |= ((data as u16) & 0b0011_1111) << 8;
                        nes.ppu.write_toggle = true;
                    }

                },
                // PPUDATA
                7 => {
                    let ppu_addr = nes.ppu.current_vram_address;
                    if nes.ppu.rendering_enabled() && 
                    (nes.ppu.current_scanline == 261 ||
                    nes.ppu.current_scanline <= 239) {
                        // Glitchy increment, a fine y and a coarse x 
                        nes.ppu.increment_coarse_x();
                        nes.ppu.increment_fine_y();
                    } else {
                        // Normal incrementing behavior based on PPUCTRL
                        if nes.ppu.control & 0x04 == 0 {
                            nes.ppu.current_vram_address += 1;
                        } else {
                            nes.ppu.current_vram_address += 32;
                        }
                        nes.ppu.current_vram_address &= 0b0111_1111_1111_1111;
                    }
                    nes.ppu.write_byte(&mut *nes.mapper, ppu_addr, data);

                    // Perform a dummy access immediately, to simulte the behavior of the PPU
                    // address lines changing, so the mapper can react accordingly
                    let address = nes.ppu.current_vram_address;
                    nes.mapper.access_ppu(address);
                },
                _ => ()
            }
        },
        0x4000 ..= 0x4013 => {
            nes.apu.write_register(address, data);
        },
        0x4014 => {
            // OAM DMA, for cheating just do this instantly and return
            // OR NOT!
            //let read_address = (data as u16) << 8;
            //for i in 0 .. 256 {
            //    let byte = read_byte(nes, read_address + i);
            //    nes.ppu.oam[i as usize] = byte;
            //}
            nes.cpu.oam_dma_address = (data as u16) << 8;
            nes.cpu.oam_dma_cycle = 0;
            nes.cpu.oam_dma_active = true;
        },
        0x4015 => {
            nes.apu.write_register(address, data);
        },
        0x4016 => {
            // Input latch
            nes.input_latch = data & 0x1 != 0;
            if nes.input_latch {
                nes.p1_data = nes.p1_input;
                nes.p2_data = nes.p2_input;
            }
        },
        0x4017 => {
            nes.apu.write_register(address, data);
        },
        _ => () // Do nothing!
    }
}
