// Set of helper functions to assist mappers with a few of the most
// common mirroring modes. Less common mirroring modes and more complex
// logic may still be implemented within individual mappers as needed.
const NT_OFFSET: (u16, u16, u16, u16) = (0x000, 0x400, 0x800, 0xC00);

pub fn horizontal_mirroring(read_address: u16) -> u16 {
    let nt_base = read_address & 0xFFF;
    let nt_address = read_address & 0x3FF;
    match nt_base {
        // Nametable 0 (top-left)
        0x000 ..= 0x3FF => nt_address + NT_OFFSET.0,
        0x400 ..= 0x7FF => nt_address + NT_OFFSET.0,
        0x800 ..= 0xBFF => nt_address + NT_OFFSET.1,
        0xC00 ..= 0xFFF => nt_address + NT_OFFSET.1,
        _ => return 0, // wat
    }
}

pub fn vertical_mirroring(read_address: u16) -> u16 {
    let nt_base = read_address & 0xFFF;
    let nt_address = read_address & 0x3FF;
    match nt_base {
        // Nametable 0 (top-left)
        0x000 ..= 0x3FF => nt_address + NT_OFFSET.0,
        0x400 ..= 0x7FF => nt_address + NT_OFFSET.1,
        0x800 ..= 0xBFF => nt_address + NT_OFFSET.0,
        0xC00 ..= 0xFFF => nt_address + NT_OFFSET.1,
        _ => return 0, // wat
    }
}

pub fn one_screen_lower(read_address: u16) -> u16 {
    let nt_base = read_address & 0xFFF;
    let nt_address = read_address & 0x3FF;
    match nt_base {
        // Nametable 0 (top-left)
        0x000 ..= 0x3FF => nt_address + NT_OFFSET.0,
        0x400 ..= 0x7FF => nt_address + NT_OFFSET.0,
        0x800 ..= 0xBFF => nt_address + NT_OFFSET.0,
        0xC00 ..= 0xFFF => nt_address + NT_OFFSET.0,
        _ => return 0, // wat
    }
}

pub fn one_screen_upper(read_address: u16) -> u16 {
    let nt_base = read_address & 0xFFF;
    let nt_address = read_address & 0x3FF;
    match nt_base {
        // Nametable 0 (top-left)
        0x000 ..= 0x3FF => nt_address + NT_OFFSET.1,
        0x400 ..= 0x7FF => nt_address + NT_OFFSET.1,
        0x800 ..= 0xBFF => nt_address + NT_OFFSET.1,
        0xC00 ..= 0xFFF => nt_address + NT_OFFSET.1,
        _ => return 0, // wat
    }
}

pub fn four_banks(read_address: u16) -> u16 {
    let nt_base = read_address & 0xFFF;
    let nt_address = read_address & 0x3FF;
    match nt_base {
        // Nametable 0 (top-left)
        0x000 ..= 0x3FF => nt_address + NT_OFFSET.0,
        0x400 ..= 0x7FF => nt_address + NT_OFFSET.1,
        0x800 ..= 0xBFF => nt_address + NT_OFFSET.2,
        0xC00 ..= 0xFFF => nt_address + NT_OFFSET.3,
        _ => return 0, // wat
    }
}
