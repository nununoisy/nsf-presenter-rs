// Implements a rolling buffer for audio samples, with a fixed length and infinite operation.
// Indices wrap around from the end of this buffer back to the beginning, so no memory allocation
// is needed once it's been constructed.

// Not intended to be generic, or particularly safe beyond rust's usual guarantees.

pub struct RingBuffer {
    buffer: Vec<i16>,
    index: usize
}

impl RingBuffer {
    pub fn new(length: usize) -> RingBuffer {
        return RingBuffer {
            buffer: vec!(0i16; length),
            index: 0
        };
    }

    pub fn push(&mut self, sample: i16) {
        self.buffer[self.index] = sample;
        self.index = (self.index + 1) % self.buffer.len();
    }

    pub fn buffer(&self) -> &Vec<i16> {
        return &self.buffer;
    }

    pub fn index(&self) -> usize {
        return self.index;
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}