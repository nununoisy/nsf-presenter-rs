# NSFPresenter

NSFPresenter is a tool I wrote to generate visualizations of my
[Dn-FamiTracker][1] covers, based on [RusticNES][2] and [FFmpeg][3].
You can see it in action on [my YouTube channel][4]. I also wrote it
to learn how to write Rust (so please forgive the code quality).

## Functionality

NSFPresenter essentially runs your input NSF through RusticNES and
sends the piano roll window's canvas and the emulated audio to FFmpeg
to be encoded as a video.

It supports NSF modules and some features of NSF2 modules. The output
format is not very customizable (since FFmpeg is not easy to set up),
but it should work for most usecases.

## Features

- Supports NSF and NSF2 modules.
- Customized version of RusticNES:
  - Added FDS audio support.
  - Slight performance enhancements for NSF playback.
- Outputs a video file:
  - Customizable resolution (default 1080p) at 60.10 FPS (the NES'/Famicom's true framerate).
  - MPEG-4 container with fast-start (`moov` atom at beginning of file).
  - yuv420p H.264 video stream encoded with libx264, crf: 16.
  - Mono AAC LC audio stream encoded with FFmpeg's aac encoder, bitrate: 192k.
- Video files are suitable for direct upload to YouTube or Discord (w/ Nitro).
- Video files have metadata based on NSF metadata (title, artist, copyright, track index).
- Loop detection for FamiTracker NSF exports.
- NSF2 features:
  - Support for extended metadata - no more 32-character limits!
  - Support for individual title/artist fields for each song in a multi-track NSF.
  - Support for NSFe duration field.
  - Support for custom mixing is planned but not yet available.

## Installation

**Windows**: head to the Releases page and grab the latest binary release. Simply unzip
             and run the executable, and you're all set.

**Linux**: no binaries yet, but you can compile from source. You'll need to have `ffmpeg`
           and `fltk` development packages installed, then clone the repo and run
           `cargo build --release` to build.

## Usage

TODO

[1]: https://github.com/Dn-Programming-Core-Management/Dn-FamiTracker
[2]: https://github.com/zeta0134/rusticnes-core
[3]: https://github.com/FFmpeg/FFmpeg
[4]: https://youtube.com/@nununoisy
