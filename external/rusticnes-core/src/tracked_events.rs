#[derive(Clone, Copy)]
pub enum EventType {
    NullEvent,
    CpuRead{program_counter: u16, address: u16, data: u8},
    CpuWrite{program_counter: u16, address: u16, data: u8},
    CpuExecute{program_counter: u16, data: u8},
}

#[derive(Clone, Copy)]
pub struct TrackedEvent {
    pub scanline: u16,
    pub cycle: u16,
    pub event_type: EventType,
}

pub struct EventTracker {
    pub tracked_events_a: Vec<TrackedEvent>,
    pub size_a: usize,
    pub tracked_events_b: Vec<TrackedEvent>,
    pub size_b: usize,
    pub a_active: bool,
    pub current_scanline: u16,
    pub current_cycle: u16,
    pub cpu_snoop_list: Vec<u8>,
}

const CPU_READ: u8    = 0b0000_0001;
const CPU_WRITE: u8   = 0b0000_0010;
const CPU_EXECUTE: u8 = 0b0000_0100;

impl EventTracker {
    pub fn new() -> EventTracker {
        let mut default_cpu_snoops = vec![0u8; 0x10000];

        // PPU registers
        default_cpu_snoops[0x2000] = CPU_WRITE;
        default_cpu_snoops[0x2001] = CPU_WRITE;
        default_cpu_snoops[0x2002] = CPU_WRITE | CPU_READ;
        default_cpu_snoops[0x2003] = CPU_WRITE;
        default_cpu_snoops[0x2004] = CPU_WRITE | CPU_READ;
        default_cpu_snoops[0x2005] = CPU_WRITE;
        default_cpu_snoops[0x2006] = CPU_WRITE;
        default_cpu_snoops[0x2007] = CPU_WRITE | CPU_READ;

        // APU registers
        // Pulse 1
        default_cpu_snoops[0x4000] = CPU_WRITE;
        default_cpu_snoops[0x4001] = CPU_WRITE;
        default_cpu_snoops[0x4002] = CPU_WRITE;
        default_cpu_snoops[0x4003] = CPU_WRITE;

        // Pulse 2
        default_cpu_snoops[0x4004] = CPU_WRITE;
        default_cpu_snoops[0x4005] = CPU_WRITE;
        default_cpu_snoops[0x4006] = CPU_WRITE;
        default_cpu_snoops[0x4007] = CPU_WRITE;

        // Triangle
        default_cpu_snoops[0x4008] = CPU_WRITE;
        default_cpu_snoops[0x400A] = CPU_WRITE;
        default_cpu_snoops[0x400B] = CPU_WRITE;

        // Noise
        default_cpu_snoops[0x400C] = CPU_WRITE;
        default_cpu_snoops[0x400E] = CPU_WRITE;
        default_cpu_snoops[0x400F] = CPU_WRITE;

        // DMC
        default_cpu_snoops[0x4010] = CPU_WRITE;
        default_cpu_snoops[0x4011] = CPU_WRITE;
        default_cpu_snoops[0x4012] = CPU_WRITE;
        default_cpu_snoops[0x4013] = CPU_WRITE;

        // Misc
        default_cpu_snoops[0x4014] = CPU_WRITE;
        default_cpu_snoops[0x4015] = CPU_WRITE | CPU_READ;
        default_cpu_snoops[0x4017] = CPU_WRITE;



        return EventTracker {
            // Way, way more events than we could *possibly* need, just to be safe
            // Manually indexed, and never resized, to avoid allocations at runtime
            tracked_events_a: vec![TrackedEvent{scanline: 0xFFFF, cycle: 0xFFFF, event_type: EventType::NullEvent}; 262*341],
            size_a: 0,
            tracked_events_b: vec![TrackedEvent{scanline: 0xFFFF, cycle: 0xFFFF, event_type: EventType::NullEvent}; 262*341],
            size_b: 0,
            a_active: true,
            current_scanline: 0,
            current_cycle: 0,
            cpu_snoop_list: default_cpu_snoops,
        }
    }

    pub fn track(&mut self, event: TrackedEvent) {
        match self.a_active {
            true => {
                self.tracked_events_a[self.size_a] = event;
                self.size_a += 1;
            },
            false => {
                self.tracked_events_b[self.size_b] = event;
                self.size_b += 1;  
            }
        }
    }

    pub fn swap_buffers(&mut self) {
        match self.a_active {
            true => {
                self.size_b = 0;
                self.a_active = false;
            },
           false => {
                self.size_a = 0;
                self.a_active = true;
            }
        }
    }

    pub fn events_this_frame(&self) -> &[TrackedEvent] {
        match self.a_active {
            true => &self.tracked_events_a[..self.size_a],
            false => &self.tracked_events_b[..self.size_b],
        }
    }

    pub fn events_last_frame(&self) -> &[TrackedEvent] {
        match self.a_active {
            true => &self.tracked_events_b[..self.size_b],
            false => &self.tracked_events_a[..self.size_a],
        }
    }

    pub fn snoop_cpu_read(&mut self, program_counter: u16, address: u16, data: u8) {
        if (self.cpu_snoop_list[address as usize] & CPU_READ) != 0 {
            self.track(TrackedEvent{
                scanline: self.current_scanline,
                cycle: self.current_cycle,
                event_type: EventType::CpuRead{
                    program_counter: program_counter,
                    address: address,
                    data: data,
                }
            });
        }
    }

    pub fn snoop_cpu_write(&mut self, program_counter: u16, address: u16, data: u8) {
        if (self.cpu_snoop_list[address as usize] & CPU_WRITE) != 0 {
            self.track(TrackedEvent{
                scanline: self.current_scanline,
                cycle: self.current_cycle,
                event_type: EventType::CpuWrite{
                    program_counter: program_counter,
                    address: address,
                    data: data,
                }
            });
        }
    }

    pub fn snoop_cpu_execute(&mut self, program_counter: u16, data: u8) {
        if (self.cpu_snoop_list[program_counter as usize] & CPU_EXECUTE) != 0 {
            self.track(TrackedEvent{
                scanline: self.current_scanline,
                cycle: self.current_cycle,
                event_type: EventType::CpuExecute{
                    program_counter: program_counter,
                    data: data,
                }
            });
        }
    }
}