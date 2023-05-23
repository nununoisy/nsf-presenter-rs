/// Represents one contiguous block of memory, typically residing on a single
/// physical chip. Implementations have varying behavior, but provide one
/// consistent guarantee: all memory access will return some value, possibly
/// open bus. This helps the trait to correctly represent missing
/// chips, wrapping behavior, mirroring, bank switching, etc.
#[derive(Clone)]
pub struct MemoryBlock {
    bytes: Vec<u8>,
    readonly: bool,
    volatile: bool
}

#[derive(PartialEq)]
pub enum MemoryType {
    Rom,
    Ram,
    NvRam,
}

impl MemoryBlock {
    pub fn new(data: &[u8], memory_type: MemoryType) -> MemoryBlock {
        return MemoryBlock {
            bytes: data.to_vec(),
            readonly: memory_type == MemoryType::Rom,
            volatile: memory_type != MemoryType::NvRam,
        }
    }

    pub fn len(&self) -> usize {
        return self.bytes.len();
    }

    pub fn is_volatile(&self) -> bool {
        return self.volatile;
    }

    pub fn is_readonly(&self) -> bool {
        return self.readonly;
    }

    pub fn bounded_read(&self, address: usize) -> Option<u8> {
        if address >= self.len() {
            return None;
        }
        return Some(self.bytes[address]);
    }

    pub fn bounded_write(&mut self, address: usize, data: u8) {
        if address >= self.len() || self.readonly  {
            return;
        }
        self.bytes[address] = data;
    }

    pub fn wrapping_read(&self, address: usize) -> Option<u8> {
        if self.bytes.len() == 0 {
            return None;
        }
        return Some(self.bytes[address % self.len()]);
    }

    pub fn wrapping_write(&mut self, address: usize, data: u8) {
        if self.bytes.len() == 0 || self.readonly {
            return;
        }
        let len = self.len();
        self.bytes[address % len] = data;
    }

    pub fn banked_read(&self, bank_size: usize, bank_index: usize, offset: usize) -> Option<u8> {
        let effective_address = (bank_size * bank_index) + (offset % bank_size);
        return self.wrapping_read(effective_address);
    }

    pub fn banked_write(&mut self, bank_size: usize, bank_index: usize, offset: usize, data: u8) {
        let effective_address = (bank_size * bank_index) + (offset % bank_size);
        self.wrapping_write(effective_address, data);
    }

    pub fn as_vec(&self) -> &Vec<u8> {
        return &self.bytes;
    }

    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        return &mut self.bytes;
    }
}

