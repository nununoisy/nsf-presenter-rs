use clap::{arg, ArgAction, value_parser, Command};
use std::path::PathBuf;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::time::Instant;
use std::time::Duration;
use indicatif::{FormattedDuration, HumanBytes, ProgressBar, ProgressStyle};
use crate::renderer::{Renderer, options::{RendererOptions, StopCondition}};

fn get_renderer_options() -> RendererOptions {
    let matches = Command::new("NSFPresenter")
        .arg(arg!(-c --"video-codec" <CODEC> "Set the output video codec")
            .required(false)
            .default_value("libx264"))
        .arg(arg!(-C --"audio-codec" <CODEC> "Set the output audio codec")
            .required(false)
            .default_value("aac"))
        .arg(arg!(-f --"pixel-format" <FORMAT> "Set the output video pixel format")
            .required(false)
            .default_value("yuv420p"))
        .arg(arg!(-F --"sample-format" <FORMAT> "Set the output audio sample format")
            .required(false)
            .default_value("fltp"))
        .arg(arg!(-R --"sample-rate" <RATE> "Set the output audio sample rate")
            .required(false)
            .value_parser(value_parser!(i32))
            .default_value("44100"))
        .arg(arg!(-T --"nsf-track" <TRACK> "Select the 1-indexed NSF track to play")
            .required(false)
            .value_parser(value_parser!(u8))
            .default_value("1"))
        .arg(arg!(-s --"stop-at" <CONDITION> "Set the stop condition")
            .required(false)
            .value_parser(value_parser!(StopCondition))
            .default_value("time:300"))
        .arg(arg!(-S --"stop-fadeout" <FRAMES> "Set the audio fadeout length in frames")
            .required(false)
            .value_parser(value_parser!(u64))
            .default_value("180"))
        .arg(arg!(--"ow" <WIDTH> "Set the output video width")
            .required(false)
            .value_parser(value_parser!(u32))
            .default_value("1920"))
        .arg(arg!(--"oh" <HEIGHT> "Set the output video height")
            .required(false)
            .value_parser(value_parser!(u32))
            .default_value("1080"))
        .arg(arg!(-J --"famicom" "Simulate the Famicom's filter chain instead of the NES'.")
            .action(ArgAction::SetTrue))
        .arg(arg!(-L --"lq-filters" "Use low-quality filter chain. Speeds up renders but has dirtier sound.")
            .action(ArgAction::SetTrue))
        .arg(arg!(-X --"multiplexing" "Emulate multiplexing for audio mixing (e.g. w/ N163). More accurate, but can introduce sound artifacts.")
            .action(ArgAction::SetTrue))
        .arg(arg!(<nsf> "NSF to render")
            .value_parser(value_parser!(PathBuf))
            .required(true))
        .arg(arg!(<output> "Output video file")
            .value_parser(value_parser!(PathBuf))
            .required(true))
        .get_matches();

    let mut options = RendererOptions::default();

    options.input_path = matches.get_one::<PathBuf>("nsf")
        .expect("Input path required")
        .to_str()
        .unwrap()
        .to_string();

    options.video_options.output_path = matches.get_one::<PathBuf>("output")
        .expect("Output path required")
        .to_str()
        .unwrap()
        .to_string();

    options.video_options.video_codec = matches.get_one::<String>("video-codec")
        .cloned()
        .unwrap();
    options.video_options.audio_codec = matches.get_one::<String>("audio-codec")
        .cloned()
        .unwrap();
    options.video_options.pixel_format_out = matches.get_one::<String>("pixel-format")
        .cloned()
        .unwrap();
    options.video_options.sample_format_out = matches.get_one::<String>("sample-format")
        .cloned()
        .unwrap();

    let sample_rate = matches.get_one::<i32>("sample-rate")
        .cloned()
        .unwrap();
    options.video_options.sample_rate = sample_rate;
    options.video_options.audio_time_base = (1, sample_rate).into();

    options.track_index = matches.get_one::<u8>("nsf-track")
        .cloned()
        .unwrap();
    options.stop_condition = matches.get_one::<StopCondition>("stop-at")
        .cloned()
        .unwrap();
    options.fadeout_length = matches.get_one::<u64>("stop-fadeout")
        .cloned()
        .unwrap();

    let ow = matches.get_one::<u32>("ow")
        .cloned()
        .unwrap();
    let oh = matches.get_one::<u32>("oh")
        .cloned()
        .unwrap();
    options.video_options.resolution_out = (ow, oh);

    options.famicom = matches.get_flag("famicom");
    options.high_quality = !(matches.get_flag("lq-filters"));
    options.multiplexing = matches.get_flag("multiplexing");

    options
}

pub fn run() {
    let options = get_renderer_options();
    let mut renderer = Renderer::new(options).unwrap();

    let pb = ProgressBar::new(0);
    let pb_style_initial = ProgressStyle::with_template("{msg}\n{spinner} Running until duration is known...")
        .unwrap();
    let pb_style = ProgressStyle::with_template("{msg}\n{wide_bar} {percent}%")
        .unwrap();
    pb.set_style(pb_style_initial);

    renderer.start_encoding().unwrap();

    loop {
        if !renderer.step().unwrap() {
            break;
        }

        if pb.length().unwrap() == 0 {
            if let Some(duration) = renderer.expected_duration_frames() {
                pb.set_length(duration as u64);
                pb.set_style(pb_style.clone());
            }
        }
        pb.set_position(renderer.cur_frame());

        let current_video_duration = FormattedDuration(renderer.encoded_duration());
        let current_video_size = HumanBytes(renderer.encoded_size() as u64);
        let current_encode_rate = renderer.encode_rate();
        let expected_video_duration = match renderer.expected_duration() {
            Some(duration) => FormattedDuration(duration).to_string(),
            None => "?".to_string()
        };
        let elapsed_duration = FormattedDuration(renderer.elapsed()).to_string();
        let eta_duration = match renderer.eta_duration() {
            Some(duration) => FormattedDuration(duration).to_string(),
            None => "?".to_string()
        };

        let mut message: String = "VID]".to_string();
        write!(message, " enc_time={}/{}", current_video_duration, expected_video_duration).unwrap();
        write!(message, " size={}", current_video_size).unwrap();
        write!(message, " rate={:.2}", current_encode_rate).unwrap();

        write!(message, "\nEMU]").unwrap();
        write!(message, " {}", renderer.emulator_progress().unwrap()).unwrap();
        write!(message, " fps={} avg_fps={}", renderer.instantaneous_fps(), renderer.average_fps()).unwrap();

        write!(message, "\nTIM]").unwrap();
        write!(message, " run_time={}/{}", elapsed_duration, eta_duration).unwrap();

        pb.set_message(message);
    }

    pb.finish_with_message("Finalizing encode...");
    renderer.finish_encoding().unwrap();

    println!("Done!");
}
