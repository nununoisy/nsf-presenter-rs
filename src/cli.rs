use std::collections::HashMap;
use clap::{arg, ArgAction, value_parser, Command};
use std::path::PathBuf;
use std::fmt::Write as _;
use indicatif::{FormattedDuration, HumanBytes, ProgressBar, ProgressStyle};
use rusticnes_ui_common::piano_roll_window::ChannelSettings;
use rusticnes_ui_common::drawing;
use csscolorparser::Color as CssColor;
use crate::renderer::{Renderer, options::{RendererOptions, StopCondition}};
use crate::emulator::Emulator;

fn get_default_channel_settings() -> HashMap<(String, String), ChannelSettings> {
    let mut emulator = Emulator::new();
    emulator.init(None);

    emulator.channel_settings()
}

fn color_value_parser(s: &str) -> Result<drawing::Color, String> {
    let parsed_color = s.parse::<CssColor>()
        .map_err(|e| e.to_string())?;

    Ok(drawing::Color::rgba(
        (parsed_color.r * 255.0) as u8,
        (parsed_color.g * 255.0) as u8,
        (parsed_color.b * 255.0) as u8,
        (parsed_color.a * 255.0) as u8
    ))
}

fn codec_option_value_parser(s: &str) -> Result<(String, String), String> {
    let (key, value) = s.split_once('=')
        .ok_or("Invalid option specification (must be of the form 'option=value').".to_string())?;

    Ok((key.to_string(), value.to_string()))
}

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
        .arg(arg!(-o --"video-option" <OPTION> "Pass an option to the video codec (option=value)")
            .required(false)
            .value_parser(codec_option_value_parser)
            .action(ArgAction::Append))
        .arg(arg!(-O --"audio-option" <OPTION> "Pass an option to the audio codec (option=value)")
            .required(false)
            .value_parser(codec_option_value_parser)
            .action(ArgAction::Append))
        .arg(arg!(-k --"channel-color" "Set the colors for a channel.")
            .required(false)
            .num_args(3..=18)
            .value_names(&["CHIP", "CHANNEL", "COLORS..."])
            .action(ArgAction::Append))
        .arg(arg!(-H --"hide-channel" "Hide a channel from the visualization.")
            .required(false)
            .num_args(2)
            .value_names(&["CHIP", "CHANNEL"])
            .action(ArgAction::Append))
        .arg(arg!(-i --"import-config" <CONFIGFILE> "Import configuration from a RusticNES TOML file.")
             .value_parser(value_parser!(PathBuf))
            .required(false))
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

    if let Some(video_options) = matches.get_many::<(String, String)>("video-option") {
        for (k, v) in video_options.cloned() {
            options.video_options.video_codec_params.insert(k, v);
        }
    }
    if let Some(audio_options) = matches.get_many::<(String, String)>("audio-option") {
        for (k, v) in audio_options.cloned() {
            options.video_options.audio_codec_params.insert(k, v);
        }
    }

    options.channel_settings = get_default_channel_settings();

    if let Some(channel_settings) = matches.get_occurrences::<String>("channel-color") {
        for channel_setting_parts in channel_settings.map(Iterator::collect::<Vec<&String>>) {
            let chip = channel_setting_parts.get(0)
                .expect("Channel setting must have chip name")
                .clone()
                .clone();
            let channel = channel_setting_parts.get(1)
                .expect("Channel setting must have channel name")
                .clone()
                .clone();

            let setting = options.channel_settings.get_mut(&(chip.clone(), channel.clone()))
                .expect(format!("Unknown chip/channel specified: {} {}", chip.clone(), channel.clone()).as_str());

            if setting.colors.len() != channel_setting_parts.len() - 2 {
                panic!("Wrong number of colors specified for chip/channel {} {}: expected {} colors", chip.clone(), channel.clone(), setting.colors.len());
            }
            setting.colors = channel_setting_parts.iter()
                .skip(2)
                .map(|c| color_value_parser(c.as_str()).expect("Invalid color"))
                .collect();
        }
    }

    if let Some(hidden_channels) = matches.get_occurrences::<String>("hide-channel") {
        for hidden_channel_parts in hidden_channels.map(Iterator::collect::<Vec<&String>>) {
            let chip = hidden_channel_parts.get(0)
                .expect("Hidden channel must have chip name")
                .clone()
                .clone();
            let channel = hidden_channel_parts.get(1)
                .expect("Hidden channel must have channel name")
                .clone()
                .clone();

            let setting = options.channel_settings.get_mut(&(chip.clone(), channel.clone()))
                .expect(format!("Unknown chip/channel specified: {} {}", chip.clone(), channel.clone()).as_str());

            setting.hidden = true;
        }
    }

    options.config_import_path = matches.get_one::<PathBuf>("import-config")
        .map(|p| p.to_str().unwrap().to_string());

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
        pb.set_position(renderer.current_frame());

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
