// The mapper used for NSF playback. This is planned to behave like a hardware
// player, so it will have some inherent limitations similar to most flashcarts.
// Reference capabilities: https://wiki.nesdev.com/w/index.php/NSF

use apu::AudioChannelState;
use asm::*;
use asm::Opcode::*;
use asm::AddressingMode::*;
use memoryblock::MemoryBlock;
use memoryblock::MemoryType;
use mmc::mapper::*;
use mmc::mirroring;
use nsf::NsfFile;
use nsf::NsfHeader;

// various expansion audio chips
use mmc::vrc6::Vrc6PulseChannel;
use mmc::vrc6::Vrc6SawtoothChannel;

use apu::PulseChannelState;
use mmc::mmc5::Mmc5PcmChannel;

use mmc::fme7::YM2149F;

use mmc::n163::Namco163Audio;
use mmc::n163::n163_mixing_level;

use mmc::vrc7::Vrc7Audio;

use mmc::fds::FdsChannel;

const PPUCTRL: u16 = 0x2000;
const PPUMASK: u16 = 0x2001;
const PPUSTATUS: u16 = 0x2002;
const PPUSCROLL: u16 = 0x2005;
const PPUADDR: u16 = 0x2006;
const PPUDATA: u16 = 0x2007;

const APUSTATUS: u16 = 0x4015;
const APUFRAMECTRL: u16 = 0x4017;

const COLOR_BLACK: u8 = 0x0F;
const COLOR_DARK_GREY: u8 = 0x2D;
const COLOR_LIGHT_GREY: u8 = 0x10;
const COLOR_WHITE: u8 = 0x30;

const COLOR_DARK_ORANGE: u8 = 0x07;
const COLOR_LIGHT_ORANGE: u8 = 0x17;
const COLOR_REALLY_LIGHT_ORANGE: u8 = 0x27;

const PLAYER_COUNTER_COMPARE: u16 = 0x01FF;
const PLAYER_BUTTON_SCRATCH: u16 = 0x01FE;
const PLAYER_PLAYBACK_COUNTER: u16 = 0x4900;
const PLAYER_TRACK_SELECT: u16 = 0x4901;
const PLAYER_CURRENT_TRACK: u16 = 0x01FD;
const PLAYER_BUTTON_REPORT: u16 = 0x4902;
const PLAYER_RESET_BANKS: u16 = 0x4903;
const PLAYER_ORIGIN: u16 = 0x4A00;
const PLAYER_SIZE: u16 = 0x0200;
const PLAYER_END: u16 = PLAYER_ORIGIN + PLAYER_SIZE - 1;

const JOYPAD1: u16 = 0x4016;

//const BUTTON_A: u8      = 1 << 7;
//const BUTTON_B: u8      = 1 << 6;
//const BUTTON_SELECT: u8 = 1 << 5;
//const BUTTON_START: u8  = 1 << 4;
const BUTTON_UP: u8     = 1 << 3;
const BUTTON_DOWN: u8   = 1 << 2;
const BUTTON_LEFT: u8   = 1 << 1;
const BUTTON_RIGHT: u8  = 1 << 0;

pub fn amplitude_from_db(db: f32) -> f32 {
    return f32::powf(10.0, db / 20.0);
}

fn wait_for_ppu_ready() -> Opcode {
    return List(vec![
        Label(String::from("vwait1")),
        // Wait for NMI twice (PPU is not ready before this)
        Bit(Absolute(PPUSTATUS)),
        Bpl(RelativeLabel(String::from("vwait1"))),
        Label(String::from("vwait2")),
        Bit(Absolute(PPUSTATUS)),
        Bpl(RelativeLabel(String::from("vwait2"))),
    ]);
}

fn initialize_ppu() -> Opcode {
    return List(vec![
        // We're in NMI now, so let's load in a better palette in slot 0
        Lda(Immediate(0x3F)),
        Sta(Absolute(PPUADDR)),
        Lda(Immediate(0x00)),
        Sta(Absolute(PPUADDR)),

        // Font color: white
        Lda(Immediate(COLOR_BLACK)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_DARK_GREY)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_LIGHT_GREY)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_WHITE)),
        Sta(Absolute(PPUDATA)),

        // Font color: yellow
        Lda(Immediate(COLOR_BLACK)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_DARK_ORANGE)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_LIGHT_ORANGE)),
        Sta(Absolute(PPUDATA)),
        Lda(Immediate(COLOR_REALLY_LIGHT_ORANGE)),
        Sta(Absolute(PPUDATA)),

        // Disable NMI, then set the scroll position and enable rendering
        Lda(Immediate(0b0000_1000)),
        Sta(Absolute(PPUCTRL)),
        Lda(Immediate(0x00)),
        Sta(Absolute(PPUSCROLL)),
        Sta(Absolute(PPUSCROLL)),
        Lda(Immediate(0b0000_1110)),
        Sta(Absolute(PPUMASK)),
    ]);
}

fn initialize_apu() -> Opcode {
    return List(vec![
        Label(String::from("initialize_apu")),
        // Disable / reset all channels
        Lda(Immediate(0x00)),
        Sta(Absolute(APUSTATUS)),

        Ldx(Immediate(0x14)),
        Label(String::from("_apu_reset_loop")),
        Dex,
        Sta(AbsoluteX(0x4000)),
        Bne(RelativeLabel(String::from("_apu_reset_loop"))),

        // Enable all channels)
        Lda(Immediate(0x0F)),
        Sta(Absolute(APUSTATUS)),
        // Set the frame counter to 4-step mode
        Lda(Immediate(0x40)),
        Sta(Absolute(APUFRAMECTRL)),
        Rts,
    ]);
}

fn initialize_memory() -> Opcode {
    return List(vec![
        Label(String::from("initialize_memory")),

        // Main memory
        Lda(Immediate(0x00)), // Start at 0x200
        Sta(ZeroPage(0x00)),
        Lda(Immediate(0x02)),
        Sta(ZeroPage(0x01)),

        Lda(Immediate(0x00)),
        Ldy(Immediate(0x00)),
        Label(String::from("_main_ram_loop")),
        Sta(IndirectIndexedY(0x00)),
        Inc(ZeroPage(0x00)),
        Bne(RelativeLabel(String::from("_main_ram_loop"))),
        Inc(ZeroPage(0x01)),
        Ldx(ZeroPage(0x01)),
        Cpx(Immediate(0x08)), // Stop when we reach 0x800
        Bne(RelativeLabel(String::from("_main_ram_loop"))),

        // PRG RAM
        Lda(Immediate(0x00)), // Start at 0x6000
        Sta(ZeroPage(0x00)),
        Lda(Immediate(0x60)),
        Sta(ZeroPage(0x01)),

        Lda(Immediate(0x00)),
        Ldy(Immediate(0x00)),
        Label(String::from("_prg_ram_loop")),
        Sta(IndirectIndexedY(0x00)),
        Inc(ZeroPage(0x00)),
        Bne(RelativeLabel(String::from("_prg_ram_loop"))),
        Inc(ZeroPage(0x01)),
        Ldx(ZeroPage(0x01)),
        Cpx(Immediate(0x80)), // Stop when we reach 0x8000
        Bne(RelativeLabel(String::from("_prg_ram_loop"))),

        // zero page
        Lda(Immediate(0x00)),
        Ldx(Immediate(0x00)),
        Label(String::from("_zero_page_loop")),
        Sta(AbsoluteX(0x0000)),
        Dex,
        Bne(RelativeLabel(String::from("_zero_page_loop"))),

        Rts,
    ]);
}

fn init_track(init_address: u16) -> Opcode {
    return List(vec![
        // (bank initialization is handled by the mapper)
        // Load the first song index to A
        Lda(Absolute(PLAYER_TRACK_SELECT)),
        Sta(Absolute(PLAYER_CURRENT_TRACK)),
        // Indicate NTSC mode in X
        Ldx(Immediate(0x00)),
        Jsr(Absolute(init_address)),
    ]);
}

fn poll_input() -> Opcode {
    return List(vec![
        // Repeatedly attempt the joypad read until we get the same value twice
        // works around a DPCM conflict
        Label(String::from("readjoy_safe")),
        Jsr(AbsoluteLabel(String::from("readjoy"))),
        Label(String::from("reread")),
        Lda(Absolute(PLAYER_BUTTON_SCRATCH)),
        Pha,
        Jsr(AbsoluteLabel(String::from("readjoy"))),
        Pla,
        Cmp(Absolute(PLAYER_BUTTON_SCRATCH)),
        Bne(RelativeLabel(String::from("reread"))),
        // Now the value in our scratch register is valid, so report it to
        // the mapper
        Sta(Absolute(PLAYER_BUTTON_REPORT)),
        Rts,

        // The joypad reading function; on its own this would be fine
        // if it weren't for DPCM
        Label(String::from("readjoy")),
        Lda(Immediate(0x01)),
        Sta(Absolute(JOYPAD1)),
        Sta(Absolute(PLAYER_BUTTON_SCRATCH)),
        Lsr(Accumulator),
        Sta(Absolute(JOYPAD1)),
        Label(String::from("joypadloop")),
        Lda(Absolute(JOYPAD1)),
        Lsr(Accumulator),
        Rol(Absolute(PLAYER_BUTTON_SCRATCH)),
        Bcc(RelativeLabel(String::from("joypadloop"))),
        Rts
    ]);
}

fn switch_tracks(init_address: u16) -> Opcode  {
    return List(vec![
        Label(String::from("switch_tracks")),
        Lda(Absolute(PLAYER_TRACK_SELECT)),
        Cmp(Absolute(PLAYER_CURRENT_TRACK)),
        Beq(RelativeLabel(String::from("done_switching_tracks"))),
        // save the current track which we are about to switch to
        Sta(Absolute(PLAYER_CURRENT_TRACK)),
        // Reset the banks prior to the init call
        // (The value written here is unimportant)
        Sta(Absolute(PLAYER_RESET_BANKS)),
        // Re-initialize the system, to clear any mode / state changes from
        // the previous track
        Jsr(AbsoluteLabel(String::from("initialize_apu"))),
        Jsr(AbsoluteLabel(String::from("initialize_memory"))),
        // load X for NTSC mode and call Init with the new track number
        Ldx(Immediate(0x00)),
        Lda(Absolute(PLAYER_CURRENT_TRACK)),
        Jsr(Absolute(init_address)),
        Label(String::from("done_switching_tracks")),
        Rts
    ]);
}

fn playback_loop(play_address: u16) -> Opcode {
    return List(vec![
        // setup playback counter wait condition
        Lda(Absolute(PLAYER_PLAYBACK_COUNTER)),
        Sta(Absolute(PLAYER_COUNTER_COMPARE)),
        // push a 0x00 byte to the stack; this will become our preserved value of A
        Lda(Immediate(0x00)),
        Pha,
        Label(String::from("playback_loop")),
        // wait for the playback counter in the mapper to change to the next value
        Lda(Absolute(PLAYER_PLAYBACK_COUNTER)),
        Cmp(Absolute(PLAYER_COUNTER_COMPARE)),
        Beq(RelativeLabel(String::from("playback_loop"))),
        Sta(Absolute(PLAYER_COUNTER_COMPARE)),
        // Pop A off the stack, and call the play address
        Pla,
        Jsr(Absolute(play_address)), // not yet
        // Preserve A, since we are about to clobber it
        Pha,
        // Poll for input (clobbers only A)
        Jsr(AbsoluteLabel(String::from("readjoy_safe"))),
        Jsr(AbsoluteLabel(String::from("switch_tracks"))),
        // All done!
        Jmp(AbsoluteLabel(String::from("playback_loop"))),
    ]);
}

fn nsf_player(init_address: u16, play_address: u16) -> Vec<Opcode> {
    vec![
        // Disable IRQ-based interrupts (We don't need them; NSF code by spec
        // shouldn't use them, and if it does, shenanigans.)
        Sei,
        // Setup the stack frame at 0x01F0 (we'll use 0x01FF for our own single variable)
        Ldx(Immediate(0xF0)),
        Txs,

        wait_for_ppu_ready(),
        initialize_ppu(),
        Jsr(AbsoluteLabel(String::from("initialize_apu"))),
        init_track(init_address),

        // This loop will never exit, it drives the playback indefinitely
        playback_loop(play_address),

        // subroutines
        poll_input(),
        switch_tracks(init_address),
        initialize_apu(),
        initialize_memory(),
    ]
}

enum TrackAdvanceMode {
    Timer,
    Silence,
    Manual
}

pub struct NsfMapper {
    prg: MemoryBlock,
    prg_ram: Vec<u8>,
    chr: Vec<u8>,
    nsf_player: Vec<u8>,
    header: NsfHeader,

    // player state, mostly used to drive the GUI and switch tracks
    current_track: u8,
    advance_mode: TrackAdvanceMode,
    current_cycles: u64,
    fade_cycles: u64,
    max_cycles: u64,
    current_sample: f32, // used for silence detection
    last_sample: f32,
    silence_counter: u64,
    silence_threshold: u64,
    gui_row: u8,

    // input shadows, populated by 6502 code
    p1_held: u8,
    p1_pressed: u8,

    prg_rom_banks: Vec<usize>,
    playback_accumulator: f32,
    playback_period: f32,
    playback_counter: u8,

    mirroring: Mirroring,
    vram: Vec<u8>,

    vrc6_enabled: bool,
    vrc6_pulse1: Vrc6PulseChannel,
    vrc6_pulse2: Vrc6PulseChannel,
    vrc6_sawtooth: Vrc6SawtoothChannel,

    mmc5_enabled: bool,
    mmc5_multiplicand_a: u8,
    mmc5_multiplicand_b: u8,
    mmc5_pulse_1: PulseChannelState,
    mmc5_pulse_2: PulseChannelState,
    mmc5_audio_sequencer_counter: u16,
    mmc5_pcm_channel: Mmc5PcmChannel,
    mmc5_exram: Vec<u8>,

    s5b_enabled: bool,
    s5b_audio_command_select: u8,
    s5b_expansion_audio_chip: YM2149F,

    n163_enabled: bool,
    n163_ram_addr: u8,
    n163_ram_auto_increment: bool,
    n163_expansion_audio_chip: Namco163Audio,
    n163_mix: f32,

    vrc7_enabled: bool,
    vrc7_audio: Vrc7Audio,
    vrc7_audio_register: u8,

    fds_enabled: bool,
    fds_channel: FdsChannel
}

impl NsfMapper {
    pub fn from_nsf(nsf: NsfFile) -> Result<NsfMapper, String> {
        let nsf_player_opcodes = nsf_player(nsf.header.init_address(), nsf.header.play_address());
        let mut nsf_player = assemble(nsf_player_opcodes, PLAYER_ORIGIN)?;
        nsf_player.resize(PLAYER_SIZE as usize, 0);

        let mut prg_rom = nsf.prg.clone();
        let mut prg_rom_banks = nsf.header.initial_banks();
        if !nsf.header.is_bank_switched() {
            if nsf.header.load_address() < 0x8000 {
                return Err(format!("Load address {} is below 0x8000, this conflicts with player implementation. Refusing to load.", nsf.header.load_address()));
            }

            // Coerce this ROM into a bank switched format anyway, so the mapper logic becomes simplified
            let mut padded_rom: Vec<u8> = Vec::new();
            padded_rom.resize((nsf.header.load_address() as usize) - 0x8000, 0);
            padded_rom.extend(prg_rom);
            padded_rom.resize(0x8000, 0);
            prg_rom = padded_rom;
            prg_rom_banks = vec![0, 1, 2, 3, 4, 5, 6, 7];
        }

        let ntsc_clockrate = 1786860.0;
        let cycles_per_play = (nsf.header.ntsc_playback_speed() as f32) * ntsc_clockrate / 1000000.0;
        let mut font_chr = include_bytes!("../../assets/troll8x8.chr").to_vec();
        font_chr.resize(0x2000, 0);

        // MMC5 pulses have no sweep unit, so we need to explicitly disable sweep muting
        let mut mmc5_pulse_1 = PulseChannelState::new("Pulse 1", "MMC5", 1_789_773, false);
        let mut mmc5_pulse_2 = PulseChannelState::new("Pulse 2", "MMC5", 1_789_773, false);
        mmc5_pulse_1.sweep_negate = true;
        mmc5_pulse_2.sweep_negate = true;

        let mut mapper = NsfMapper {
            prg: MemoryBlock::new(&prg_rom, MemoryType::Ram),
            chr: font_chr,
            nsf_player: nsf_player,
            header: nsf.header,
            playback_accumulator: 0.0,
            playback_period: cycles_per_play,
            playback_counter: 0,

            current_track: nsf.header.starting_song(),
            advance_mode: if nsf.header.total_songs() > 1 {TrackAdvanceMode::Timer} else {TrackAdvanceMode::Manual},
            current_cycles: 0,
            fade_cycles: 1_789_773 * 2,
            max_cycles: 1_789_773 * 180,
            current_sample: 0.0,
            last_sample: 0.0,
            silence_counter: 0,
            silence_threshold: 1_789_773 * 3,
            gui_row: 0,

            p1_held: 0,
            p1_pressed: 0,

            vrc6_enabled: nsf.header.vrc6(),
            vrc6_pulse1: Vrc6PulseChannel::new("Pulse 1"),
            vrc6_pulse2: Vrc6PulseChannel::new("Pulse 2"),
            vrc6_sawtooth: Vrc6SawtoothChannel::new(),

            mmc5_enabled: nsf.header.mmc5(),
            mmc5_multiplicand_a: 0,
            mmc5_multiplicand_b: 0,
            mmc5_pulse_1: mmc5_pulse_1,
            mmc5_pulse_2: mmc5_pulse_2,
            mmc5_audio_sequencer_counter: 0,
            mmc5_pcm_channel: Mmc5PcmChannel::new(),
            mmc5_exram: vec![0u8; 0x400],

            s5b_enabled: nsf.header.s5b(),
            s5b_audio_command_select: 0,
            s5b_expansion_audio_chip: YM2149F::new(),

            n163_enabled: nsf.header.n163(),
            n163_ram_addr: 0,
            n163_ram_auto_increment: false,
            n163_expansion_audio_chip: Namco163Audio::new(),
            n163_mix: n163_mixing_level(0),

            vrc7_enabled: nsf.header.vrc7(),
            vrc7_audio: Vrc7Audio::new(),
            vrc7_audio_register: 0,

            fds_enabled: nsf.header.fds(),
            fds_channel: FdsChannel::new("Wavetable"),

            prg_rom_banks: prg_rom_banks,

            mirroring: Mirroring::FourScreen,
            vram: vec![0u8; 0x1000],
            prg_ram: vec![0u8; 0x2000]
        };

        mapper.vrc6_write(0x9003, 0x00); // some NSF files expect VRC6 to already be enabled, so do that
        mapper.fds_channel.nsf_init();
        return Ok(mapper);
    }

    pub fn draw_string(&mut self, x: usize, y: usize, width: usize, chars: Vec<u8>) {
        let mut dx = x;
        for c in chars {
            if dx < 32 && dx < (x + width) {
                if c >= 32 && c <= 127 {
                    let tile = y * 32 + dx;
                    let index = c - 32;
                    self.vram[tile] = index;
                }
            }
            dx += 1;
        }
    }

    pub fn set_tile(&mut self, x: usize, y: usize, index: u8) {
        let tile = y * 32 + x;
        self.vram[tile] = index;
    }

    pub fn clear_display(&mut self) {
        for i in 0 .. 1024 {
            self.vram[i] = 0;
        }
        // palette things
        for i in 0x3C8 ..= 0x3E0 {
            self.vram[i] = 0b0000_0101;
        }
    }

    pub fn progress_bar(&mut self, x: usize, y: usize, width: usize, progress: f32, out_of: f32) {
        // ends
        self.set_tile(x, y, 0x79);
        self.set_tile(x + width - 1, y, 0x7A);
        // middle bits

        let tile_width = width - 2;
        let effective_progress = progress / out_of; // ranging from 0 - 1
        for i in 0 .. tile_width {
            let tile_min = (i as f32) / (tile_width as f32);
            let tile_max = ((i + 1) as f32) / (tile_width as f32);
            // simple cases: this tile is before / after the middle point
            if effective_progress <= tile_min {
                self.set_tile(x + i + 1, y, 0x70);
            } else if effective_progress >= tile_max {
                self.set_tile(x + i + 1, y, 0x78);
            } else {
                // complicated case: how far are we through this specific tile?
                let tile_progress = (effective_progress - tile_min) * (tile_width as f32);
                let index = ((tile_progress * 8.0) as u8).min(0x7).max(0x0) + 0x71;
                self.set_tile(x + i + 1, y, index);
            }
        }
    }

    pub fn update_display(&mut self) {
        self.clear_display();

        self.draw_string(21, 2, 9,  "RusticNES".as_bytes().to_vec());
        self.draw_string(20, 3, 10, "NSF Player".as_bytes().to_vec());

        self.draw_string(2, 5, 28, "Title".as_bytes().to_vec());
        let song_name = self.header.song_name();
        self.draw_string(2, 6, 28, song_name);

        self.draw_string(2, 9, 28, "Artist".as_bytes().to_vec());
        let artist_name = self.header.artist_name();
        self.draw_string(2, 10, 28, artist_name);

        self.draw_string(2, 13, 28, "Copyright".as_bytes().to_vec());
        let copyright_holder = self.header.copyright_holder();
        self.draw_string(2, 14, 28, copyright_holder);

        let current_seconds = self.current_cycles / 1_789_773;
        let max_seconds = self.max_cycles / 1_789_773;

        let track_display = if self.header.total_songs() <= 1 {
            format!("{}", self.current_track)
        } else {
            format!("{}  /  {}", self.current_track, self.header.total_songs())
        };
        
        self.draw_string(4, 20, 6, "Track:".as_bytes().to_vec());
        self.draw_string(12, 20, track_display.len(), track_display.as_bytes().to_vec());

        if self.header.total_songs() <= 1 {
            return;
        }

        let advance_mode_string = match self.advance_mode {
            TrackAdvanceMode::Timer => "After Length",
            TrackAdvanceMode::Silence => "After Silence",
            TrackAdvanceMode::Manual => "Manual"
        };
        let advance_display = format!("Next:   {}", advance_mode_string);
        self.draw_string(4, 22, advance_display.len(), advance_display.as_bytes().to_vec());

        let track_play_time = format!("{}:{:02}", current_seconds / 60, current_seconds % 60);
        let max_play_time = format!("{}:{:02}", max_seconds / 60, max_seconds % 60);

        if matches!(self.advance_mode, TrackAdvanceMode::Timer) {
            self.draw_string(4, 24, 8, "Length: ".as_bytes().to_vec());
            self.draw_string(12, 24, max_play_time.len(), max_play_time.as_bytes().to_vec());
        }

        self.draw_string(2, (20 + self.gui_row * 2) as usize, 1, ">".as_bytes().to_vec());

        match self.advance_mode {
            TrackAdvanceMode::Timer => {
                let duration_display = format!("{} / {}", track_play_time, max_play_time);
                self.draw_string(19, 27, duration_display.len(), duration_display.as_bytes().to_vec());
                self.progress_bar(1, 27, 17, self.current_cycles as f32, self.max_cycles as f32);
            },
            TrackAdvanceMode::Silence => {
                let duration_display = format!("{}", track_play_time);
                self.draw_string(26, 27, duration_display.len(), duration_display.as_bytes().to_vec());
                // fudge this just slightly; chop off the first 1% or so, to avoid a flicker. (silence detection
                // can be a bit noisy, ironically)
                let one_percent = (self.silence_threshold as f32) * 0.01;
                let fudged_counter = ((self.silence_counter  as f32) - one_percent).max(0.0);
                let fudged_threshold = (self.silence_threshold  as f32) - one_percent;
                self.progress_bar(1, 27, 24, fudged_counter, fudged_threshold);
            },
            TrackAdvanceMode::Manual => {
                let duration_display = format!("{}", track_play_time);
                self.draw_string(26, 27, duration_display.len(), duration_display.as_bytes().to_vec());
            },
        }
    }

    pub fn process_input(&mut self) {
        match self.gui_row {
            0 => {
            /* Track select row */
              if (self.p1_pressed & BUTTON_RIGHT) != 0 {
                    if self.current_track < self.header.total_songs() {
                        self.current_track += 1;
                        self.current_cycles = 0;
                    }
                }
                if (self.p1_pressed & BUTTON_LEFT) != 0 {
                    if self.current_track > 1 {
                       self.current_track -= 1;
                       self.current_cycles = 0;
                    }
                }
                if (self.p1_pressed & BUTTON_DOWN) != 0 && self.header.total_songs() > 1 {
                    self.gui_row += 1;
                }
            },
            /* advance mode select row */
            1 => {
                if (self.p1_pressed & BUTTON_UP) != 0 {
                    self.gui_row -= 1;
                }
                if (self.p1_pressed & BUTTON_RIGHT) != 0  {
                    if matches!(self.advance_mode, TrackAdvanceMode::Silence) {
                        self.advance_mode = TrackAdvanceMode::Manual
                    }
                    if matches!(self.advance_mode, TrackAdvanceMode::Timer) {
                        self.advance_mode = TrackAdvanceMode::Silence
                    }
                }
                if (self.p1_pressed & BUTTON_LEFT) != 0 {
                    if matches!(self.advance_mode, TrackAdvanceMode::Silence) {
                        self.advance_mode = TrackAdvanceMode::Timer
                    }
                    if matches!(self.advance_mode, TrackAdvanceMode::Manual) {
                        self.advance_mode = TrackAdvanceMode::Silence
                    }
                }
                if (self.p1_pressed & BUTTON_DOWN) != 0  && matches!(self.advance_mode, TrackAdvanceMode::Timer) {
                    self.gui_row += 1;
                }

            },
            /* timer duration row */
            2 => {
                if (self.p1_pressed & BUTTON_UP) != 0 {
                    self.gui_row -= 1;
                }
                if (self.p1_pressed & BUTTON_RIGHT) != 0  {
                    self.max_cycles += 1_789_773 * 30;
                }
                if (self.p1_pressed & BUTTON_LEFT) != 0 && self.max_cycles > 1_789_773 * 30 {
                    self.max_cycles -= 1_789_773 * 30;
                }
            },
            _ => {}
        }
    }

    pub fn advance_track_with_wraparound(&mut self) {
        if self.current_track < self.header.total_songs() {
            self.current_track += 1;
        } else {
            self.current_track = 1;
        }
        self.current_cycles = 0;
    }

    pub fn update_player(&mut self) {
        match self.advance_mode {
            TrackAdvanceMode::Timer => {
                if self.current_cycles > self.max_cycles {
                    self.advance_track_with_wraparound();
                }
                // *also* advance when the silence threshold is passed, for short tracks in an otherwise
                // loopy album
                if self.silence_counter > self.silence_threshold {
                    self.advance_track_with_wraparound();
                    self.silence_counter = 0;
                }
            },
            TrackAdvanceMode::Silence => {
                if self.silence_counter > self.silence_threshold {
                    self.advance_track_with_wraparound();
                    self.silence_counter = 0;
                }
            }
        _ => {/* do nothing! */},
        }
    }

    pub fn update_gui(&mut self) {
        self.process_input();
        self.update_player();
        self.update_display();
    }

    pub fn vrc6_output(&self) -> f32 {
        if !self.vrc6_enabled {
            return 0.0;
        }
        let pulse_1_output = if !self.vrc6_pulse1.debug_disable {self.vrc6_pulse1.output() as f32} else {0.0};
        let pulse_2_output = if !self.vrc6_pulse2.debug_disable {self.vrc6_pulse2.output() as f32} else {0.0};
        let sawtooth_output = if !self.vrc6_sawtooth.debug_disable {self.vrc6_sawtooth.output() as f32} else {0.0};
        let vrc6_combined_sample = (pulse_1_output + pulse_2_output + sawtooth_output) / 61.0;

        let nes_pulse_full_volume = 95.88 / ((8128.0 / 15.0) + 100.0);
        let vrc6_pulse_full_volume = 15.0 / 61.0;
        let vrc6_weight = nes_pulse_full_volume / vrc6_pulse_full_volume;
        return vrc6_combined_sample * vrc6_weight;
    }

    pub fn vrc6_write(&mut self, address: u16, data: u8) {
        match address {
            0x9000 => {self.vrc6_pulse1.write_register(0, data);},
            0x9001 => {self.vrc6_pulse1.write_register(1, data);},
            0x9002 => {self.vrc6_pulse1.write_register(2, data);},
            0x9003 => {
                self.vrc6_pulse1.write_register(3, data);
                self.vrc6_pulse2.write_register(3, data);
                self.vrc6_sawtooth.write_register(3, data);
            },
            0xA000 => {self.vrc6_pulse2.write_register(0, data);},
            0xA001 => {self.vrc6_pulse2.write_register(1, data);},
            0xA002 => {self.vrc6_pulse2.write_register(2, data);},
            // no 0xA003
            0xB000 => {self.vrc6_sawtooth.write_register(0, data);},
            0xB001 => {self.vrc6_sawtooth.write_register(1, data);},
            0xB002 => {self.vrc6_sawtooth.write_register(2, data);},
            _ => {}
        }
    }

    pub fn clock_vrc6(&mut self) {
        if self.vrc6_enabled {
            self.vrc6_pulse1.clock();
            self.vrc6_pulse2.clock();
            self.vrc6_sawtooth.clock();
        }
    }

    pub fn mmc5_write(&mut self, address: u16, data: u8) {
        if !self.mmc5_enabled {
            return;
        }
        let duty_table = [
            0b1000_0000,
            0b1100_0000,
            0b1111_0000,
            0b0011_1111,
        ];
        match address {
            0x5000 => {
                let duty_index =      (data & 0b1100_0000) >> 6;
                let length_disable =  (data & 0b0010_0000) != 0;
                let constant_volume = (data & 0b0001_0000) != 0;

                self.mmc5_pulse_1.duty = duty_table[duty_index as usize];
                self.mmc5_pulse_1.length_counter.halt_flag = length_disable;
                self.mmc5_pulse_1.envelope.looping = length_disable;
                self.mmc5_pulse_1.envelope.enabled = !(constant_volume);
                self.mmc5_pulse_1.envelope.volume_register = data & 0b0000_1111;
            },
            0x5002 => {
                let period_low = data as u16;
                self.mmc5_pulse_1.period_initial = (self.mmc5_pulse_1.period_initial & 0xFF00) | period_low;
            },
            0x5003 => {
                let period_high =  ((data & 0b0000_0111) as u16) << 8;
                let length_index = (data & 0b1111_1000) >> 3;

                self.mmc5_pulse_1.period_initial = (self.mmc5_pulse_1.period_initial & 0x00FF) | period_high;
                self.mmc5_pulse_1.length_counter.set_length(length_index);

                // Start this note
                self.mmc5_pulse_1.sequence_counter = 0;
                self.mmc5_pulse_1.envelope.start_flag = true;
            },
            0x5004 => {
                let duty_index =      (data & 0b1100_0000) >> 6;
                let length_disable =  (data & 0b0010_0000) != 0;
                let constant_volume = (data & 0b0001_0000) != 0;

                self.mmc5_pulse_2.duty = duty_table[duty_index as usize];
                self.mmc5_pulse_2.length_counter.halt_flag = length_disable;
                self.mmc5_pulse_2.envelope.looping = length_disable;
                self.mmc5_pulse_2.envelope.enabled = !(constant_volume);
                self.mmc5_pulse_2.envelope.volume_register = data & 0b0000_1111;
            },
            0x5006 => {
                let period_low = data as u16;
                self.mmc5_pulse_2.period_initial = (self.mmc5_pulse_2.period_initial & 0xFF00) | period_low;
            },
            0x5007 => {
                let period_high =  ((data & 0b0000_0111) as u16) << 8;
                let length_index =  (data & 0b1111_1000) >> 3;

                self.mmc5_pulse_2.period_initial = (self.mmc5_pulse_2.period_initial & 0x00FF) | period_high;
                self.mmc5_pulse_2.length_counter.set_length(length_index);

                // Start this note
                self.mmc5_pulse_2.sequence_counter = 0;
                self.mmc5_pulse_2.envelope.start_flag = true;
            },
            0x5010 => {
                self.mmc5_pcm_channel.read_mode =  (data & 0b0000_0001) != 0;
                self.mmc5_pcm_channel.irq_enable =  (data & 0b1000_0000) != 0;
            },
            0x5011 => {
                if !(self.mmc5_pcm_channel.read_mode) {
                    self.mmc5_pcm_channel.level = data;
                }
            },
            0x5015 => {
                self.mmc5_pulse_1.length_counter.channel_enabled  = (data & 0b0001) != 0;
                self.mmc5_pulse_2.length_counter.channel_enabled  = (data & 0b0010) != 0;
              
                if !(self.mmc5_pulse_1.length_counter.channel_enabled) {
                    self.mmc5_pulse_1.length_counter.length = 0;
                }
                if !(self.mmc5_pulse_2.length_counter.channel_enabled) {
                    self.mmc5_pulse_2.length_counter.length = 0;
                }
            }
            0x5205 => {self.mmc5_multiplicand_a = data;},
            0x5206 => {self.mmc5_multiplicand_b = data;},
            0x5C00 ..= 0x5FF5 => {
                self.mmc5_exram[(address - 0x5C00) as usize] = data;
            }
            _ => {}
        }
    }

    fn clock_mmc5(&mut self) {
        if !self.mmc5_enabled {
            return;
        }
        self.mmc5_audio_sequencer_counter += 1;
        if (self.mmc5_audio_sequencer_counter & 0b1) == 0 {
            self.mmc5_pulse_1.clock();
            self.mmc5_pulse_2.clock();
        }
        if self.mmc5_audio_sequencer_counter >= 7446 {
            self.mmc5_pulse_1.envelope.clock();
            self.mmc5_pulse_2.envelope.clock();
            self.mmc5_pulse_1.length_counter.clock();
            self.mmc5_pulse_2.length_counter.clock();
            // Note: MMC5 pulse channels don't support sweep. We're borrowing the implementation
            // from the underlying APU, but intentionally not clocking the sweep units.
            self.mmc5_audio_sequencer_counter = 0;
        }
    }

    fn snoop_mmc5(&mut self, address: u16) {
        if !self.mmc5_enabled {
            return;
        }
        // do the snoop PCM playback thing
        if self.mmc5_pcm_channel.read_mode {
            match address {
                0x8000 ..= 0xBFFF => {
                    self.mmc5_pcm_channel.level = self.debug_read_cpu(address).unwrap_or(0);
                },
                _ => {}
            }
        }
    }

    fn read_mmc5(&self, address: u16) -> Option<u8> {
        if !self.mmc5_enabled {
            return None;
        }

        // Handle MMC5 specific address spaces
        match address {
            0x5010 => {
                let mut pcm_status = 0;
                if self.mmc5_pcm_channel.read_mode {
                    pcm_status |= 0b0000_0001;
                }
                if self.mmc5_pcm_channel.irq_pending {
                    pcm_status |= 0b1000_0000;   
                }
                return Some(pcm_status)
            },
            0x5015 => {
                let mut pulse_status = 0;
                if self.mmc5_pulse_1.length_counter.length > 0 {
                    pulse_status += 0b0000_0001;
                }
                if self.mmc5_pulse_2.length_counter.length > 0 {
                    pulse_status += 0b0000_0010;
                }
                return Some(pulse_status);
            },
            0x5205 => {
                let result = self.mmc5_multiplicand_a as u16 * self.mmc5_multiplicand_b as u16;
                return Some((result & 0xFF) as u8);
            },
            0x5206 => {
                let result = self.mmc5_multiplicand_a as u16 * self.mmc5_multiplicand_b as u16;
                return Some(((result & 0xFF00) >> 8) as u8);
            },
            0x5C00 ..= 0x5FF5 => {
                return Some(self.mmc5_exram[(address - 0x5C00) as usize]);
            }
            _ => return None
        }
    }

    fn mmc5_output(&self) -> f32 {
        if !self.mmc5_enabled {
            return 0.0;
        }
        
        let pulse_1_output = if !self.mmc5_pulse_1.debug_disable {(self.mmc5_pulse_1.output() as f32 / 15.0) - 0.5} else {0.0};
        let pulse_2_output = if !self.mmc5_pulse_2.debug_disable {(self.mmc5_pulse_2.output() as f32 / 15.0) - 0.5} else {0.0};
        let pcm_output = if !self.mmc5_pcm_channel.muted {(self.mmc5_pcm_channel.level as f32 / 256.0) - 0.5} else {0.0};

        return 
            (pulse_1_output + pulse_2_output) * 0.12 + 
            pcm_output * 0.25;
    }

    fn s5b_write(&mut self, address: u16, data: u8) {
        if !self.s5b_enabled {
            return;
        }
        match address {
            0xC000 => {
                self.s5b_audio_command_select = data & 0x0F;
            },
            0xE000 => {
                self.s5b_expansion_audio_chip.execute_command(self.s5b_audio_command_select, data);
            }
            _ => {}
        }
    }

    fn s5b_output(&self) -> f32 {
        if !self.s5b_enabled {
            return 0.0;
        }
        return (self.s5b_expansion_audio_chip.output() - 0.5) * -1.06;
    }

    fn clock_s5b(&mut self) {
        if !self.s5b_enabled {
            return;
        }
        self.s5b_expansion_audio_chip.clock();
    }

    fn n163_write(&mut self, address: u16, data: u8) {
        if !self.n163_enabled {
            return;
        }
        match address {
            0x4800 => {
                self.n163_expansion_audio_chip.internal_ram[self.n163_ram_addr as usize] = data;
                if self.n163_ram_auto_increment {
                    self.n163_ram_addr = (self.n163_ram_addr + 1) & 0x7F;
                }
            },
            0xF800 => {
                self.n163_ram_addr = data & 0x7F;
                self.n163_ram_auto_increment = (data & 0b1000_0000) != 0;
            },
            _ => {}
        }
    }

    fn n163_read(&self, address: u16) -> Option<u8> {
        if !self.n163_enabled {
            return None;
        }
        match address {
            0x4800 => {
                Some(self.n163_expansion_audio_chip.internal_ram[self.n163_ram_addr as usize])
            },
            _ => None
        }
    }

    fn n163_snoop(&mut self, address: u16) {
        if !self.n163_enabled {
            return;
        }
        match address {
            0x4800 => {
                if self.n163_ram_auto_increment {
                    self.n163_ram_addr = (self.n163_ram_addr + 1) & 0x7F;
                }
            },
            _ => {}
        }
    }

    fn n163_output(&self) -> f32 {
        if !self.n163_enabled {
            return 0.0;
        }

        // APU pulse numbers from https://wiki.nesdev.com/w/index.php?title=APU_Mixer
        let nes_pulse_full_volume = 95.88 / ((8128.0 / 15.0) + 100.0);
        let n163_square_full_volume = 15.0 * 15.0; // loudest sample * loudest volume
        
        // Normalize the N163 volume against APU pulse, then multiply that by our
        // desired relative mix:
        let n163_weight = (nes_pulse_full_volume / n163_square_full_volume) * self.n163_mix;

        return self.n163_expansion_audio_chip.current_output * n163_weight;
    }

    fn clock_n163(&mut self) {
        if !self.n163_enabled {
            return;
        }
        self.n163_expansion_audio_chip.clock();
    }

    pub fn vrc7_output(&self) -> f32 {
        if !self.vrc7_enabled {
            return 0.0;
        }
        let combined_vrc7_audio = self.vrc7_audio.output() as f32 / 256.0 / 6.0;

        let stock_vrc7_db = 6.23;
        let desired_vrc7_db = 11.00;
        let mixed_vrc7_audio = combined_vrc7_audio * amplitude_from_db(desired_vrc7_db - stock_vrc7_db);

        return mixed_vrc7_audio;
    }

    pub fn clock_vrc7(&mut self) {
        if !self.vrc7_enabled {
            return;
        }
        self.vrc7_audio.clock();
    }

    pub fn vrc7_write(&mut self, address: u16, data: u8) {
        if !self.vrc7_enabled {
            return;
        }
        match address {
            0x9010  => {
                self.vrc7_audio_register = data
            },
            0x9030          => {
                self.vrc7_audio.write(self.vrc7_audio_register, data);
            },
            _ => {}
        }
    }

    fn fds_write(&mut self, address: u16, data: u8) {
        if !self.fds_enabled {
            return;
        }
        match address {
            0x4040..=0x407F => {
                if self.fds_channel.wave_table.write_enable {
                    self.fds_channel.wave_table.table[address as usize & 0x3F] = data;
                }
            },
            0x4080 => self.fds_channel.vol_envelope.write_config_register(data),
            0x4082 => self.fds_channel.wave_table.write_freq_low(data),
            0x4083 => self.fds_channel.wave_table.write_freq_high(data),
            0x4084 => self.fds_channel.mod_envelope.write_config_register(data),
            0x4085 => self.fds_channel.mod_table.pos = data as u32,
            0x4086 => self.fds_channel.mod_table.write_freq_low(data),
            0x4087 => self.fds_channel.mod_table.write_freq_high(data),
            0x4088 => {
                if self.fds_channel.mod_table.mod_halt {
                    for _ in 0..2 {
                        let wave_idx = (self.fds_channel.mod_table.phase >> 16) & 0x3F;
                        self.fds_channel.mod_table.table[wave_idx] = data & 0x7;
                        self.fds_channel.mod_table.phase += 0x010000;
                        self.fds_channel.mod_table.phase &= 0x3FFFFF;
                    }
                }
            },
            0x4089 => self.fds_channel.wave_table.write_config_register(data),
            0x408A => {
                self.fds_channel.master_envelope_speed = data;
                self.fds_channel.vol_envelope.timer = 0;
                self.fds_channel.mod_envelope.timer = 0;
            },
            _ => ()
        };
    }

    fn fds_read(&self, address: u16) -> Option<u8> {
        if !self.fds_enabled {
            return None;
        }
        match address {
            0x4040..=0x407F => Some(self.fds_channel.wave_table.table[address as usize & 0x3F]),
            0x4090 => Some(self.fds_channel.vol_envelope.out | 0x40),
            0x4092 => Some(self.fds_channel.mod_envelope.out | 0x40),
            _ => None
        }
    }

    fn fds_output(&self) -> f32 {
        if !self.fds_enabled {
            return 0.0;
        }

        // APU pulse numbers from https://wiki.nesdev.com/w/index.php?title=APU_Mixer
        let nes_pulse_full_volume = 95.88 / ((8128.0 / 15.0) + 100.0);
        let fds_full_volume = 32.0 * 63.0;

        let fds_weight = (nes_pulse_full_volume / fds_full_volume) * 2.4f32;

        self.fds_channel.current_volume * fds_weight
    }

    fn clock_fds(&mut self) {
        if !self.fds_enabled {
            return;
        }
        self.fds_channel.clock();
    }

    fn fade_weight(&self) -> f32 {
        match self.advance_mode {
            TrackAdvanceMode::Timer => {
                let fade_start = self.max_cycles - self.fade_cycles;
                if self.current_cycles < fade_start {
                    return 1.0
                }
                let cycles_into_fade = self.current_cycles - fade_start;
                let fade_weight = (cycles_into_fade as f32) / (self.fade_cycles as f32);
                return 1.0 - fade_weight.max(0.0).min(1.0);
            },
            _ => return 1.0 // do not fade
        }
    }

    fn detect_silence(&self) -> bool {
        let delta = (self.last_sample - self.current_sample).abs();
        return delta < 0.005;
    }
}

impl Mapper for NsfMapper {
    fn nsf_set_track(&mut self, track_index: u8) {
        self.current_track = track_index;
    }

    fn nsf_manual_mode(&mut self) {
        self.advance_mode = TrackAdvanceMode::Manual;
    }

    fn mirroring(&self) -> Mirroring {
        return self.mirroring;
    }

    fn clock_cpu(&mut self) {
        self.playback_accumulator += 1.0;
        if self.playback_accumulator > self.playback_period {
            self.playback_counter = self.playback_counter.wrapping_add(1);
            self.playback_accumulator -= self.playback_period;
            self.update_gui();
        }

        self.clock_vrc6();
        self.clock_mmc5();
        self.clock_s5b();
        self.clock_n163();
        self.clock_vrc7();
        self.clock_fds();
        self.current_cycles += 1;

        if self.detect_silence() {
            self.silence_counter += 1;
        } else {
            self.silence_counter = 0;
        }
    }

    fn mix_expansion_audio(&self, nes_sample: f32) -> f32 {
        let mixed_sample =  
            self.vrc6_output() +
            self.mmc5_output() +
            self.s5b_output() +
            self.n163_output() +
            self.vrc7_output() +
            self.fds_output() +
            nes_sample;
        return mixed_sample * self.fade_weight();
    }

    fn channels(&self) ->  Vec<& dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        if self.vrc6_enabled {
            channels.push(&self.vrc6_pulse1);
            channels.push(&self.vrc6_pulse2);
            channels.push(&self.vrc6_sawtooth);
        }
        if self.mmc5_enabled {
            channels.push(&self.mmc5_pulse_1);
            channels.push(&self.mmc5_pulse_2);
            channels.push(&self.mmc5_pcm_channel);
        }
        if self.s5b_enabled {
            channels.push(&self.s5b_expansion_audio_chip.channel_a);
            channels.push(&self.s5b_expansion_audio_chip.channel_b);
            channels.push(&self.s5b_expansion_audio_chip.channel_c);
        }
        if self.n163_enabled {
            let mut n163_channels: Vec<& dyn AudioChannelState> = Vec::new();
            let enabled_channels = self.n163_expansion_audio_chip.enabled_channels();
            n163_channels.push(&self.n163_expansion_audio_chip.channel1);
            n163_channels.push(&self.n163_expansion_audio_chip.channel2);
            n163_channels.push(&self.n163_expansion_audio_chip.channel3);
            n163_channels.push(&self.n163_expansion_audio_chip.channel4);
            n163_channels.push(&self.n163_expansion_audio_chip.channel5);
            n163_channels.push(&self.n163_expansion_audio_chip.channel6);
            n163_channels.push(&self.n163_expansion_audio_chip.channel7);
            n163_channels.push(&self.n163_expansion_audio_chip.channel8);
            n163_channels.truncate(enabled_channels);
            channels.append(&mut n163_channels);
        }
        if self.vrc7_enabled {
            let mut vrc7_channels: Vec<& dyn AudioChannelState> = Vec::new();
            vrc7_channels.push(&self.vrc7_audio.channel1);
            vrc7_channels.push(&self.vrc7_audio.channel2);
            vrc7_channels.push(&self.vrc7_audio.channel3);
            vrc7_channels.push(&self.vrc7_audio.channel4);
            vrc7_channels.push(&self.vrc7_audio.channel5);
            vrc7_channels.push(&self.vrc7_audio.channel6);
            channels.append(&mut vrc7_channels);
        }
        if self.fds_enabled {
            channels.push(&self.fds_channel);
        }
        return channels;
    }

    fn channels_mut(&mut self) ->  Vec<&mut dyn AudioChannelState> {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        if self.vrc6_enabled {
            channels.push(&mut self.vrc6_pulse1);
            channels.push(&mut self.vrc6_pulse2);
            channels.push(&mut self.vrc6_sawtooth);
        }
        if self.mmc5_enabled {
            channels.push(&mut self.mmc5_pulse_1);
            channels.push(&mut self.mmc5_pulse_2);
            channels.push(&mut self.mmc5_pcm_channel);
        }
        if self.s5b_enabled {
            channels.push(&mut self.s5b_expansion_audio_chip.channel_a);
            channels.push(&mut self.s5b_expansion_audio_chip.channel_b);
            channels.push(&mut self.s5b_expansion_audio_chip.channel_c);
        }
        if self.n163_enabled {
            let mut n163_channels: Vec<&mut dyn AudioChannelState> = Vec::new();
            let enabled_channels = self.n163_expansion_audio_chip.enabled_channels();
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel1);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel2);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel3);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel4);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel5);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel6);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel7);
            n163_channels.push(&mut self.n163_expansion_audio_chip.channel8);
            n163_channels.truncate(enabled_channels);
            channels.append(&mut n163_channels);
        }
        if self.vrc7_enabled {
            let mut vrc7_channels: Vec<&mut dyn AudioChannelState> = Vec::new();
            vrc7_channels.push(&mut self.vrc7_audio.channel1);
            vrc7_channels.push(&mut self.vrc7_audio.channel2);
            vrc7_channels.push(&mut self.vrc7_audio.channel3);
            vrc7_channels.push(&mut self.vrc7_audio.channel4);
            vrc7_channels.push(&mut self.vrc7_audio.channel5);
            vrc7_channels.push(&mut self.vrc7_audio.channel6);
            channels.append(&mut vrc7_channels);
        }
        if self.fds_enabled {
            channels.push(&mut self.fds_channel);
        }
        return channels;
    }

    fn record_expansion_audio_output(&mut self, nes_sample: f32) {
        if self.vrc6_enabled {
            self.vrc6_pulse1.record_current_output();
            self.vrc6_pulse2.record_current_output();
            self.vrc6_sawtooth.record_current_output();
        }
        if self.mmc5_enabled {
            self.mmc5_pulse_1.record_current_output();
            self.mmc5_pulse_2.record_current_output();
            self.mmc5_pcm_channel.record_current_output();
        }
        if self.s5b_enabled {
            self.s5b_expansion_audio_chip.record_output();
        }
        if self.n163_enabled {
            self.n163_expansion_audio_chip.record_output();
        }
        if self.vrc7_enabled {
            self.vrc7_audio.record_output();
        }
        if self.fds_enabled {
            self.fds_channel.record_current_output();
        }
        self.last_sample = self.current_sample;
        self.current_sample = self.mix_expansion_audio(nes_sample);
    }
    
    fn read_cpu(&mut self, address: u16) -> Option<u8> {
        let data = self.debug_read_cpu(address);
        self.snoop_mmc5(address);
        self.n163_snoop(address);
        return data;
    }

    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match self.read_mmc5(address) {
            Some(data) => return Some(data),
            None => {}
        }

        match self.n163_read(address) {
            Some(data) => return Some(data),
            None => {}
        }

        match self.fds_read(address) {
            Some(data) => return Some(data),
            None => {}
        }

        match address {
            PLAYER_PLAYBACK_COUNTER => Some(self.playback_counter),
            PLAYER_TRACK_SELECT => Some(self.current_track - 1),
            PLAYER_ORIGIN ..= PLAYER_END => Some(self.nsf_player[(address - PLAYER_ORIGIN) as usize]),
            0x6000 ..= 0x7FFF => Some(self.prg_ram[(address - 0x6000) as usize]),
            0x8000 ..= 0x8FFF => self.prg.banked_read(0x1000, self.prg_rom_banks[0], (address - 0x8000) as usize),
            0x9000 ..= 0x9FFF => self.prg.banked_read(0x1000, self.prg_rom_banks[1], (address - 0x9000) as usize),
            0xA000 ..= 0xAFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[2], (address - 0xA000) as usize),
            0xB000 ..= 0xBFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[3], (address - 0xB000) as usize),
            0xC000 ..= 0xCFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[4], (address - 0xC000) as usize),
            0xD000 ..= 0xDFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[5], (address - 0xD000) as usize),
            0xE000 ..= 0xEFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[6], (address - 0xE000) as usize),
            0xF000 ..= 0xFFFB => self.prg.banked_read(0x1000, self.prg_rom_banks[7], (address - 0xF000) as usize),
            0xFFFC => {Some(((PLAYER_ORIGIN & 0x00FF) >> 0) as u8)}, // reset vector
            0xFFFD => {Some(((PLAYER_ORIGIN & 0xFF00) >> 8) as u8)},
            0xFFFE ..= 0xFFFF => self.prg.banked_read(0x1000, self.prg_rom_banks[7], (address - 0xF000) as usize),
            _ => None
        }
    }

    fn write_cpu(&mut self, address: u16, data: u8) {
        match address {
            PLAYER_BUTTON_REPORT => {
                self.p1_pressed = data & (!self.p1_held);
                self.p1_held = data;
            },
            PLAYER_RESET_BANKS => {
                self.prg_rom_banks = self.header.initial_banks();
                if !self.header.is_bank_switched() {
                    self.prg_rom_banks = vec![0, 1, 2, 3, 4, 5, 6, 7];
                }
            },
            0x5FF8 => {self.prg_rom_banks[0] = data as usize},
            0x5FF9 => {self.prg_rom_banks[1] = data as usize},
            0x5FFA => {self.prg_rom_banks[2] = data as usize},
            0x5FFB => {self.prg_rom_banks[3] = data as usize},
            0x5FFC => {self.prg_rom_banks[4] = data as usize},
            0x5FFD => {self.prg_rom_banks[5] = data as usize},
            0x5FFE => {self.prg_rom_banks[6] = data as usize},
            0x5FFF => {self.prg_rom_banks[7] = data as usize},
            0x6000 ..= 0x7FFF => {self.prg_ram[(address - 0x6000) as usize] = data},
            _ => {}
        }
        if self.vrc6_enabled {
            self.vrc6_write(address, data);
        }
        if self.mmc5_enabled {
            self.mmc5_write(address, data);
        }
        if self.s5b_enabled {
            self.s5b_write(address, data);
        }
        if self.n163_enabled {
            self.n163_write(address, data);
        }
        if self.vrc7_enabled {
            self.vrc7_write(address, data);
        }
        if self.fds_enabled {
            self.fds_write(address, data);
        }
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 ..= 0x1FFF => return Some(self.chr[address as usize]),
            0x2000 ..= 0x3FFF => return match self.mirroring {
                Mirroring::Horizontal => Some(self.vram[mirroring::horizontal_mirroring(address) as usize]),
                Mirroring::Vertical   => Some(self.vram[mirroring::vertical_mirroring(address) as usize]),
                Mirroring::FourScreen => Some(self.vram[mirroring::four_banks(address) as usize]),
                _ => None
            },
            _ => return None
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        match address {
            0x0000 ..= 0x1FFF => {self.chr[address as usize] = data},
            0x2000 ..= 0x3FFF => match self.mirroring {
                Mirroring::Horizontal => self.vram[mirroring::horizontal_mirroring(address) as usize] = data,
                Mirroring::Vertical   => self.vram[mirroring::vertical_mirroring(address) as usize] = data,
                _ => {}
            },
            _ => {}
        }
    }

    fn audio_multiplexing(&mut self, emulate: bool) {
        self.n163_expansion_audio_chip.emulate_multiplexing = emulate;
    }

    fn vrc7_set_patches(&mut self, patches: &[u8]) {
        self.vrc7_audio.set_patches(patches);
    }
}
