// This is an extremely simple collection of routines that can produce
// arbitrary 6502 code as a byte string. Especially useful for mappers
// which need to include arbitrary code that isn't provided by the file
// for whatever reason, but possibly also handy for units tests down
// the line.

use std::collections::HashMap;

#[derive(Clone,Debug)]
pub enum AddressingMode {
    Accumulator,
    Immediate(u8),
    ZeroPage(u8),
    ZeroPageX(u8),
    ZeroPageY(u8),
    Relative(i8),
    RelativeLabel(String),
    Absolute(u16),
    AbsoluteLabel(String),
    AbsoluteX(u16),
    AbsoluteLabelX(String),
    AbsoluteY(u16),
    AbsoluteLabelY(String),
    Indirect(u16),
    IndexedIndirectX(u8),
    IndirectIndexedY(u8),
}

#[derive(Clone,Debug)]
pub enum Opcode {
    Adc(AddressingMode),
    And(AddressingMode),
    Asl(AddressingMode),
    Bcc(AddressingMode),
    Bcs(AddressingMode),
    Beq(AddressingMode),
    Bit(AddressingMode),
    Bmi(AddressingMode),
    Bne(AddressingMode),
    Bpl(AddressingMode),
    Brk,
    Bvc(AddressingMode),
    Bvs(AddressingMode),
    Clc,
    Cli,
    Clv,
    Cmp(AddressingMode),
    Cpx(AddressingMode),
    Cpy(AddressingMode),
    Dec(AddressingMode),
    Dex,
    Dey,
    Eor(AddressingMode),
    Inc(AddressingMode),
    Inx,
    Iny,
    Jmp(AddressingMode),
    Jsr(AddressingMode),
    Label(String),
    List(Vec<Opcode>),
    Lda(AddressingMode),
    Ldx(AddressingMode),
    Ldy(AddressingMode),
    Lsr(AddressingMode),
    Nop,
    Ora(AddressingMode),
    Pha,
    Php,
    Pla,
    Plp,
    Rol(AddressingMode),
    Ror(AddressingMode),
    Rti,
    Rts,
    Sbc(AddressingMode),
    Sec,
    Sei,
    Sta(AddressingMode),
    Stx(AddressingMode),
    Sty(AddressingMode),
    Tax,
    Tay,
    Tsx,
    Txa,
    Txs,
    Tya,   
}

// Utilities to help compact the opcode decoding block
fn low(word: u16) -> u8 {
    return (word & 0x00FF) as u8;
}

fn high(word: u16) -> u8 {
    return ((word & 0xFF00) >> 8) as u8;
}

pub fn opcode_bytes(opcode: Opcode) -> Result<Vec<u8>, String> {
    match opcode {
        Opcode::Asl(AddressingMode::Accumulator) =>            {Ok(vec![0x0A])},
        Opcode::Asl(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0x06, byte])},
        Opcode::Asl(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0x16, byte])},
        Opcode::Asl(AddressingMode::Absolute(address)) =>      {Ok(vec![0x0E, low(address), high(address)])},
        Opcode::Asl(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0x1E, low(address), high(address)])},

        Opcode::Bit(AddressingMode::ZeroPage(byte)) => {Ok(vec![0x24, byte])},
        Opcode::Bit(AddressingMode::Absolute(address)) => {Ok(vec![0x2C, low(address), high(address)])},
        Opcode::Brk => {Ok(vec![0x00])},
        Opcode::Bcc(AddressingMode::Relative(offset)) => {Ok(vec![0x90, offset as u8])},
        Opcode::Bcs(AddressingMode::Relative(offset)) => {Ok(vec![0xB0, offset as u8])},
        Opcode::Beq(AddressingMode::Relative(offset)) => {Ok(vec![0xF0, offset as u8])},
        Opcode::Bmi(AddressingMode::Relative(offset)) => {Ok(vec![0x30, offset as u8])},
        Opcode::Bne(AddressingMode::Relative(offset)) => {Ok(vec![0xD0, offset as u8])},
        Opcode::Bpl(AddressingMode::Relative(offset)) => {Ok(vec![0x10, offset as u8])},
        Opcode::Clc => {Ok(vec![0x18])},
        Opcode::Cli => {Ok(vec![0x58])},

        Opcode::Cmp(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xC9, byte])},
        Opcode::Cmp(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xC5, byte])},
        Opcode::Cmp(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0xD5, byte])},
        Opcode::Cmp(AddressingMode::Absolute(address)) =>      {Ok(vec![0xCD, low(address), high(address)])},
        Opcode::Cmp(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0xDD, low(address), high(address)])},
        Opcode::Cmp(AddressingMode::AbsoluteY(address)) =>     {Ok(vec![0xD9, low(address), high(address)])},
        Opcode::Cmp(AddressingMode::IndexedIndirectX(byte)) => {Ok(vec![0xC1, byte])},
        Opcode::Cmp(AddressingMode::IndirectIndexedY(byte)) => {Ok(vec![0xD1, byte])},
        Opcode::Cpx(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xE0, byte])},
        Opcode::Cpx(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xE4, byte])},
        Opcode::Cpx(AddressingMode::Absolute(address)) =>      {Ok(vec![0xEC, low(address), high(address)])},
        Opcode::Cpy(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xC0, byte])},
        Opcode::Cpy(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xC4, byte])},
        Opcode::Cpy(AddressingMode::Absolute(address)) =>      {Ok(vec![0xCC, low(address), high(address)])},

        Opcode::Dec(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xC6, byte])},
        Opcode::Dec(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0xD6, byte])},
        Opcode::Dec(AddressingMode::Absolute(address)) =>      {Ok(vec![0xCE, low(address), high(address)])},
        Opcode::Dec(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0xDE, low(address), high(address)])},

        Opcode::Dex => {Ok(vec![0xCA])},
        Opcode::Dey => {Ok(vec![0x88])},

        Opcode::Inc(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xE6, byte])},
        Opcode::Inc(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0xF6, byte])},
        Opcode::Inc(AddressingMode::Absolute(address)) =>      {Ok(vec![0xEE, low(address), high(address)])},
        Opcode::Inc(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0xFE, low(address), high(address)])},

        Opcode::Inx => {Ok(vec![0xE8])},
        Opcode::Iny => {Ok(vec![0xC8])},

        Opcode::Lda(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xA9, byte])},
        Opcode::Lda(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xA5, byte])},
        Opcode::Lda(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0xB5, byte])},
        Opcode::Lda(AddressingMode::Absolute(address)) =>      {Ok(vec![0xAD, low(address), high(address)])},
        Opcode::Lda(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0xBD, low(address), high(address)])},
        Opcode::Lda(AddressingMode::AbsoluteY(address)) =>     {Ok(vec![0xB9, low(address), high(address)])},
        Opcode::Lda(AddressingMode::IndexedIndirectX(byte)) => {Ok(vec![0xA1, byte])},
        Opcode::Lda(AddressingMode::IndirectIndexedY(byte)) => {Ok(vec![0xB1, byte])},

        Opcode::Ldx(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xA2, byte])},
        Opcode::Ldx(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xA6, byte])},
        Opcode::Ldx(AddressingMode::ZeroPageY(byte)) =>        {Ok(vec![0xB6, byte])},
        Opcode::Ldx(AddressingMode::Absolute(address)) =>      {Ok(vec![0xAE, low(address), high(address)])},
        Opcode::Ldx(AddressingMode::AbsoluteY(address)) =>     {Ok(vec![0xBE, low(address), high(address)])},

        Opcode::Ldy(AddressingMode::Immediate(byte)) =>        {Ok(vec![0xA0, byte])},
        Opcode::Ldy(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0xA4, byte])},
        Opcode::Ldy(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0xB4, byte])},
        Opcode::Ldy(AddressingMode::Absolute(address)) =>      {Ok(vec![0xAC, low(address), high(address)])},
        Opcode::Ldy(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0xBC, low(address), high(address)])},

        Opcode::Lsr(AddressingMode::Accumulator) =>            {Ok(vec![0x4A])},
        Opcode::Lsr(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0x46, byte])},
        Opcode::Lsr(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0x56, byte])},
        Opcode::Lsr(AddressingMode::Absolute(address)) =>      {Ok(vec![0x4E, low(address), high(address)])},
        Opcode::Lsr(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0x5E, low(address), high(address)])},

        Opcode::Jmp(AddressingMode::Absolute(address)) =>      {Ok(vec![0x4C, low(address), high(address)])},
        Opcode::Jmp(AddressingMode::Indirect(address)) =>      {Ok(vec![0x6C, low(address), high(address)])},
        Opcode::Jsr(AddressingMode::Absolute(address)) =>      {Ok(vec![0x20, low(address), high(address)])},

        Opcode::Pha => {Ok(vec![0x48])},
        Opcode::Php => {Ok(vec![0x08])},
        Opcode::Pla => {Ok(vec![0x68])},
        Opcode::Plp => {Ok(vec![0x28])},

        Opcode::Rol(AddressingMode::Accumulator) =>            {Ok(vec![0x2A])},
        Opcode::Rol(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0x26, byte])},
        Opcode::Rol(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0x36, byte])},
        Opcode::Rol(AddressingMode::Absolute(address)) =>      {Ok(vec![0x2E, low(address), high(address)])},
        Opcode::Rol(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0x3E, low(address), high(address)])},

        Opcode::Ror(AddressingMode::Accumulator) =>            {Ok(vec![0x6A])},
        Opcode::Ror(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0x66, byte])},
        Opcode::Ror(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0x76, byte])},
        Opcode::Ror(AddressingMode::Absolute(address)) =>      {Ok(vec![0x6E, low(address), high(address)])},
        Opcode::Ror(AddressingMode::AbsoluteX(address)) =>     {Ok(vec![0x7E, low(address), high(address)])},

        Opcode::Rts => {Ok(vec![0x60])},
        Opcode::Rti => {Ok(vec![0x40])},

        Opcode::Sei => {Ok(vec![0x78])},
        Opcode::Sec => {Ok(vec![0x38])},

        Opcode::Tax => {Ok(vec![0xAA])},
        Opcode::Tay => {Ok(vec![0xA8])},
        Opcode::Tsx => {Ok(vec![0xBA])},
        Opcode::Txa => {Ok(vec![0x8A])},
        Opcode::Txs => {Ok(vec![0x9A])},
        Opcode::Tya => {Ok(vec![0x98])},
        
        Opcode::Sta(AddressingMode::ZeroPage(byte)) =>         {Ok(vec![0x85, byte])},
        Opcode::Sta(AddressingMode::ZeroPageX(byte)) =>        {Ok(vec![0x95, byte])},
        Opcode::Sta(AddressingMode::Absolute(address)) => {Ok(vec![0x8D, low(address), high(address)])},
        Opcode::Sta(AddressingMode::AbsoluteX(address)) => {Ok(vec![0x9D, low(address), high(address)])},
        Opcode::Sta(AddressingMode::AbsoluteY(address)) => {Ok(vec![0x99, low(address), high(address)])},
        Opcode::Sta(AddressingMode::IndexedIndirectX(byte)) => {Ok(vec![0x81, byte])},
        Opcode::Sta(AddressingMode::IndirectIndexedY(byte)) => {Ok(vec![0x91, byte])},

        opcode => {Err(format!("Unimplemented! {:<3?}", opcode))}
    }
}

fn relative_offset(known_labels: &HashMap<String, u16>, label: &String, current_address: u16) -> Result<i8, String> {
    match known_labels.get(label) {
        Some(label_address) => {
            //let current_offset = assemble(translated_opcodes.clone())?.len();
            let relative_offset = (*label_address as i32) - (current_address as i32) - 2;
            if relative_offset > 127 || relative_offset < -128 {
                return Err(format!("Branch to label {} is out of range ({})", label, relative_offset))
            }
            return Ok(relative_offset as i8);

        },
        None => return Err(format!("Label not found: {}", label))
    }
}

fn label_address(known_labels: &HashMap<String, u16>, label: &String) -> Result<u16, String> {
    match known_labels.get(label) {
        Some(address) => Ok(*address),
        None => Err(format!("Label not found: {}", label))
    }
}

pub fn resolve_labels(opcodes: Vec<Opcode>, starting_address: u16) -> Result<Vec<Opcode>, String> {
    let mut known_labels: HashMap<String, u16> = HashMap::new();
    let mut total_bytes: u16 = 0;
    for opcode in &opcodes {
        match opcode {
            Opcode::Label(label) => {
                known_labels.insert(label.to_string(), total_bytes);
            },
            // These opcodes will fail to resolve in opcode_bytes, so we instead catch them here
            // and advance the total_bytes manually; we'll replace these in a later step
            Opcode::Bcc(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Bcs(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Beq(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Bmi(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Bne(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Bpl(AddressingMode::RelativeLabel(_)) => {total_bytes += 2},
            Opcode::Jmp(AddressingMode::AbsoluteLabel(_)) => {total_bytes += 3},
            Opcode::Jsr(AddressingMode::AbsoluteLabel(_)) => {total_bytes += 3},

            opcode => {
                let bytes = opcode_bytes(opcode.clone())?;
                total_bytes += bytes.len() as u16;
            }
        }
    }

    // Now that we have our list of labels built up, we can actually apply their values
    // to the opcode list. While we're at it, we'll remove the labels tokens, as they don't map
    // to a valid byte sequence.
    let mut translated_opcodes: Vec<Opcode> = Vec::new();
    total_bytes = 0;
    for opcode in &opcodes {
        match opcode {
            Opcode::Label(_) => {},
            Opcode::Bcc(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Bcc(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Bcs(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Bcs(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Beq(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Beq(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Bmi(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Bmi(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Bne(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Bne(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Bpl(AddressingMode::RelativeLabel(label)) => {
                let offset = relative_offset(&known_labels, &label, total_bytes)?;
                translated_opcodes.push(Opcode::Bpl(AddressingMode::Relative(offset)));
                total_bytes += 2;
            },
            Opcode::Jmp(AddressingMode::AbsoluteLabel(label)) => {
                let offset = label_address(&known_labels, &label)?;
                translated_opcodes.push(Opcode::Jmp(AddressingMode::Absolute(starting_address + offset)));
                total_bytes += 3;
            },
            Opcode::Jsr(AddressingMode::AbsoluteLabel(label)) => {
                let offset = label_address(&known_labels, &label)?;
                translated_opcodes.push(Opcode::Jsr(AddressingMode::Absolute(starting_address + offset)));
                total_bytes += 3;
            },
            opcode => {
                translated_opcodes.push(opcode.clone());
                total_bytes += opcode_bytes(opcode.clone())?.len() as u16;
            },
        }
    }

    return Ok(translated_opcodes);
}

pub fn flatten(opcodes: Vec<Opcode>) -> Vec<Opcode> {
    // given a list of opcodes that may contain List<Opcode>, pack this list into a flattened
    // set of tokens. This function is recursive; do be careful.
    let mut flattened_opcodes: Vec<Opcode> = Vec::new();
    for opcode in opcodes {
        match opcode {
            Opcode::List(sublist) => {
                flattened_opcodes.extend(flatten(sublist));
            },
            opcode => {
                flattened_opcodes.push(opcode);
            }
        }
    }
    return flattened_opcodes;
}

pub fn assemble(opcodes: Vec<Opcode>, starting_address: u16) -> Result<Vec<u8>, String> {
    let mut bytes: Vec<u8> = Vec::new();
    let flattened_opcodes = flatten(opcodes);
    let translated_opcodes = resolve_labels(flattened_opcodes, starting_address)?;
    for opcode in translated_opcodes {
        bytes.extend(opcode_bytes(opcode)?);
    }
    return Ok(bytes);
}