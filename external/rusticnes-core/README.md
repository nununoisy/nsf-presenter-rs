# RusticNES-Core

This is an NES emulator written in the Rust programming language. I began this project because I wanted to teach myself Rust, and having already written [another emulator](https://github.com/zeta0134/LuaGB), I figured this was as good a way to introduce myself to the language as any.

The emulator is split up into the Core library (this repository) and platform specific shells which depend on this library. rusticnes-core contains the entire emulator with as few external dependencies as possible (presently just Rust's standard FileIO functions) so that it remains portable. All platform specific code is the responsibility of the shell.

If you're looking to compile and run a working copy of the emulator for PCs, you want [RusticNES-SDL](https://github.com/zeta0134/rusticnes-sdl), which is the reference implementation. I've tested this on Windows and Arch Linux, and it should run on Mac, and any other platform that [rust-sdl2](https://github.com/Rust-SDL2/rust-sdl2) supports. I may update this README with usage instructions for the core library after the project stabilizes a bit. At the moment the project is in constant flux and lacks what I'd call a stable API, so I'll instead refer you to [RusticNES-SDL](https://github.com/zeta0134/rusticnes-sdl) for the reference implementation.

I'm striving for cycle accuracy with the emulator. While it works and runs many games, it presently falls short of this goal. I am presently most focused on getting the base emulator to run properly, and pass various [accuracy tests](http://tasvideos.org/EmulatorResources/NESAccuracyTests.html). Mapper support should be easy to add as I go. Here is the current state of the major systems:

## 6502 CPU

- All instructions, including unofficial instructions, NOPs, and STPs
- Mostly cycle accurate, which should include additional reads / writes on dummy cycles.
- Missing proper read delay implementation, needed for DMC DMA read delay, and proper interaction between DMC and OAM DMA during simultaneous operation.

## APU

- Feature complete as far as I can tell. Pulse, Triangle, Noise, and DMC are all working properly.
- DMC wait delay is not implemented.
- Audio is not mixed properly, relative channel volumes are therefore sometimes quite incorrect. It's close enough that things sound okay unless you know what to listen for.
- No interpolation or filtering, which can make especially high frequencies sound a bit off. The APU is producing the correct output, but the subsequent clamping to 44.1 KHz introduces artifacts.
- Triangle channel intentionally does not emulate extremely high frequencies, to avoid artifacts in the handful of games that use this to "silence" the channel

## PPU

- Memory mapping, including cartridge mapper support, is all implemented and should be working.
- Nametable mirroring modes appear to work correctly, and are controlled by the mapper.
- Cycle timing should be very close to accurate. Tricky games like Battletoads appear to run correctly, though there may still be bugs here and there.
- Sprite overflow is implemented correctly. The sprite overflow bug is not, so games relying on the behavior of the sprite overflow flag will encounter accuracy problems relative to real hardware.

## Input

- A single Standard Controller plugged into port 1 is implemented. 
- Multiple controllers and additional peripheral support (Light Zapper, Track and Field Mat, Knitting Machine, etc) is planned, but not implemented.

## Mappers

- Currently supported: AxROM, CnROM, GxROM, MMC1, MMC3, NROM, PxROM, UxROM
- Currently unsupported: Everything else.
- Behavior seems mostly correct, but accuracy is not guaranteed.
- Some of blarggs mapper tests do not pass, especially those involving timing, which may be due to missing RDY line implementation
- FDS and non-NTSC NES features (PAL, Vs System, etc) are entirely unsupported.
