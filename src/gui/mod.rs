mod render_thread;

use slint;
use slint::{Color, Model};
use native_dialog::{FileDialog, MessageDialog, MessageType};
use std::rc::Rc;
use std::cell::RefCell;
use std::{fs, thread};
use std::collections::HashMap;
use std::path;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::Duration;
use indicatif::{FormattedDuration, HumanBytes};
use rusticnes_ui_common::piano_roll_window::ChannelSettings;
use rusticnes_ui_common::drawing;
use crate::emulator::{Emulator, Nsf, NsfDriverType};
use crate::gui::render_thread::RenderThreadMessage;
use crate::renderer::options::{FRAME_RATE, RendererOptions, StopCondition};
use crate::video_builder::backgrounds::VideoBackground;

slint::include_modules!();

// The return type looks wrong but it is not
fn slint_string_arr<I>(a: I) -> slint::ModelRc<slint::SharedString>
where
    I: IntoIterator<Item = String>
{
    let shared_string_vec: Vec<slint::SharedString> = a.into_iter()
        .map(|s| s.into())
        .collect();
    slint::ModelRc::new(slint::VecModel::from(shared_string_vec))
}

fn slint_int_arr<I, N>(a: I) -> slint::ModelRc<i32>
where
    N: Into<i32>,
    I: IntoIterator<Item = N>
{
    let int_vec: Vec<i32> = a.into_iter()
        .map(|n| n.into())
        .collect();
    slint::ModelRc::new(slint::VecModel::from(int_vec))
}

fn slint_color_component_arr<I: IntoIterator<Item = drawing::Color>>(a: I) -> slint::ModelRc<slint::ModelRc<i32>> {
    let color_vecs: Vec<slint::ModelRc<i32>> = a.into_iter()
        .map(|c| slint::ModelRc::new(slint::VecModel::from(vec![c.r() as i32, c.g() as i32, c.b() as i32])))
        .collect();
    slint::ModelRc::new(slint::VecModel::from(color_vecs))
}

fn get_module_metadata(path: &str) -> ModuleMetadata {
    let cart_data = fs::read(path).unwrap();
    let nsf = Nsf::from(&cart_data);
    let nsfe_metadata = nsf.nsfe_metadata();

    let (title, artist, copyright, extended_metadata) = match &nsfe_metadata {
        Some(nsfe_metadata) => {
            let title = nsfe_metadata.title().unwrap_or(nsf.title().unwrap());
            let artist = nsfe_metadata.artist().unwrap_or(nsf.artist().unwrap());
            let copyright = nsfe_metadata.copyright().unwrap_or(nsf.title().unwrap());
            (title, artist, copyright, true)
        },
        None => {
            (nsf.title().unwrap(), nsf.artist().unwrap(), nsf.copyright().unwrap(), false)
        }
    };
    let driver = match nsf.driver_type() {
        NsfDriverType::Unknown => "Unknown".to_string(),
        NsfDriverType::FTClassic => "FamiTracker".to_string(),
        NsfDriverType::FT0CC => "0CC-FamiTracker".to_string(),
        NsfDriverType::FTDn => "Dn-FamiTracker".to_string()
    };
    let loop_detection = nsf.driver_type() != NsfDriverType::Unknown;
    let extended_durations = match &nsfe_metadata {
        Some(nsfe_metadata) => {
            (0..nsf.songs())
                .map(|i| nsfe_metadata.track_duration(i as usize + 1).unwrap_or(0) as i32)
                .collect()
        },
        None => vec![]
    };

    let mut chips: Vec<String> = vec!["2A03".to_string()];
    if nsf.fds() { chips.push("FDS".to_string()); }
    if nsf.n163() { chips.push("N163".to_string()); }
    if nsf.mmc5() { chips.push("MMC5".to_string()); }
    if nsf.vrc6() { chips.push("VRC6".to_string()); }
    if nsf.vrc7() { chips.push("VRC7".to_string()); }
    if nsf.s5b() { chips.push("S5B".to_string()); }

    let tracks: Vec<String> = (0..nsf.songs())
        .map(|i| {
            if let Some(m) = &nsfe_metadata {
                if let Some(title) = m.track_title(i as usize + 1) {
                    return title;
                }
            }
            format!("Track {}", i + 1)
        })
        .collect();

    let mut result = ModuleMetadata::default();
    result.title = title.into();
    result.artist = artist.into();
    result.copyright = copyright.into();
    result.driver = driver.into();
    result.extended_metadata = extended_metadata;
    result.loop_detection = loop_detection;
    result.extended_durations = slint_int_arr(extended_durations);
    result.chips = slint_string_arr(chips);
    result.tracks = slint_string_arr(tracks);

    result
}

fn get_default_channel_settings() -> HashMap<(String, String), ChannelSettings> {
    let mut emulator = Emulator::new();
    emulator.init();

    emulator.channel_settings()
}

fn browse_for_module_dialog() -> Option<String> {
    let file = FileDialog::new()
        .add_filter("Nintendo Sound Files", &["nsf"])
        .show_open_single_file();

    match file {
        Ok(Some(path)) => Some(path.to_str().unwrap().to_string()),
        _ => None
    }
}

fn browse_for_background_dialog() -> Option<String> {
    let file = FileDialog::new()
        .add_filter("All supported formats", &["mp4", "mkv", "mov", "avi", "webm", "gif", "jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp", "qoi"])
        .add_filter("Video background formats", &["mp4", "mkv", "mov", "avi", "webm", "gif"])
        .add_filter("Image background formats", &["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp", "qoi"])
        .show_open_single_file();

    match file {
        Ok(Some(path)) => Some(path.to_str().unwrap().to_string()),
        _ => None
    }
}

fn browse_for_video_dialog() -> Option<String> {
    let file = FileDialog::new()
        .add_filter("All supported formats", &["mp4", "mkv", "mov"])
        .add_filter("MPEG-4 Video", &["mp4"])
        .add_filter("Matroska Video", &["mkv"])
        .add_filter("QuickTime Video", &["mov"])
        .show_save_single_file();

    match file {
        Ok(Some(path)) => Some(path.to_str().unwrap().to_string()),
        _ => None
    }
}

fn confirm_prores_export_dialog() -> bool {
    MessageDialog::new()
        .set_title("NSFPresenter")
        .set_text("You have chosen to export a QuickTime video. Do you want to export in ProRes 4444 format to \
                   preserve alpha information for video editing? Note that ProRes 4444 is a lossless codec, so \
                   the exported file may be very large.")
        .set_type(MessageType::Info)
        .show_confirm()
        .unwrap()
}

fn display_error_dialog(text: &str) {
    MessageDialog::new()
        .set_title("NSFPresenter")
        .set_text(text)
        .set_type(MessageType::Error)
        .show_alert()
        .unwrap();
}

pub fn run() {
    let main_window = MainWindow::new().unwrap();

    main_window.global::<ColorUtils>().on_hex_to_color(|hex| {
        let rgb = u32::from_str_radix(hex.to_string().trim_start_matches("#"), 16).unwrap_or(0);

        Color::from_argb_encoded(0xFF000000 | rgb)
    });

    main_window.global::<ColorUtils>().on_color_to_hex(|color| {
        format!("#{:02x}{:02x}{:02x}", color.red(), color.green(), color.blue()).into()
    });

    main_window.global::<ColorUtils>().on_color_components(|color| {
        slint_int_arr([color.red() as i32, color.green() as i32, color.blue() as i32])
    });

    let channel_settings = get_default_channel_settings();
    for ((chip, channel), settings) in channel_settings.iter() {
        let colors: Vec<_> = settings.colors
            .iter()
            .map(|c| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
            .collect();
        println!("{} {} - {} {}", chip, channel, settings.hidden, colors.join(", "));

        let configs_model = match chip.as_str() {
            "2A03" => main_window.get_config_2a03(),
            "MMC5" => main_window.get_config_mmc5(),
            "N163" => main_window.get_config_n163(),
            "VRC6" => main_window.get_config_vrc6(),
            "VRC7" => main_window.get_config_vrc7(),
            "YM2149F" => main_window.get_config_s5b(),
            "FDS" => main_window.get_config_fds(),
            "APU" => main_window.get_config_apu(),
            _ => continue
        };
        let mut configs: Vec<ChannelConfig> = configs_model
            .as_any()
            .downcast_ref::<slint::VecModel<ChannelConfig>>()
            .unwrap()
            .iter()
            .collect();
        if let Some(config) = configs.iter_mut().find(|cfg| channel.clone() == cfg.name.to_string()) {
            config.hidden = settings.hidden;
            config.colors = slint_color_component_arr(settings.colors.clone());
        }
        let new_config_model = slint::ModelRc::new(slint::VecModel::from(configs));
        match chip.as_str() {
            "2A03" => main_window.set_config_2a03(new_config_model),
            "MMC5" => main_window.set_config_mmc5(new_config_model),
            "N163" => main_window.set_config_n163(new_config_model),
            "VRC6" => main_window.set_config_vrc6(new_config_model),
            "VRC7" => main_window.set_config_vrc7(new_config_model),
            "YM2149F" => main_window.set_config_s5b(new_config_model),
            "FDS" => main_window.set_config_fds(new_config_model),
            "APU" => main_window.set_config_apu(new_config_model),
            _ => continue
        }
    }

    let mut options = Rc::new(RefCell::new(RendererOptions::default()));

    let (rt_handle, rt_tx) = {
        let main_window_weak = main_window.as_weak();
        render_thread::render_thread(move |msg| {
            match msg {
                RenderThreadMessage::Error(e) => {
                    slint::invoke_from_event_loop(move || {
                        let error_message = format!("Render thread reported error: {}\
                                                           \n\nThe program will now exit", e);
                        display_error_dialog(&error_message);
                        slint::quit_event_loop().unwrap();
                    }).unwrap();
                }
                RenderThreadMessage::RenderStarting => {
                    let main_window_weak = main_window_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        main_window_weak.unwrap().set_rendering(true)
                    }).unwrap();
                }
                RenderThreadMessage::RenderProgress(p) => {
                    let current_video_size = HumanBytes(p.encoded_size as u64);
                    let current_video_duration = FormattedDuration(p.encoded_duration);
                    let expected_video_duration = match p.expected_duration {
                        Some(duration) => FormattedDuration(duration).to_string(),
                        None => "?".to_string()
                    };
                    let elapsed_duration = FormattedDuration(p.elapsed_duration);
                    let eta_duration = match p.eta_duration {
                        Some(duration) => FormattedDuration(duration).to_string(),
                        None => "?".to_string()
                    };
                    let song_position = match p.song_position {
                        Some(position) => position.to_string(),
                        None => "?".to_string()
                    };
                    let loop_count = match p.loop_count {
                        Some(loops) => loops.to_string(),
                        None => "?".to_string()
                    };

                    let status_lines = vec![
                        format!(
                            "FPS: {}, Encoded: {}/{}, Output size: {}",
                            p.average_fps,
                            current_video_duration, expected_video_duration,
                            current_video_size
                        ),
                        format!(
                            "Elapsed/ETA: {}/{}, Driver position: {}, Loop count: {}",
                            elapsed_duration, eta_duration,
                            song_position,
                            loop_count
                        )
                    ];
                    let (progress, progress_bar_text) = match p.expected_duration_frames {
                        Some(exp_dur_frames) => {
                            let progress = p.frame as f64 / exp_dur_frames as f64;
                            (progress, format!("{}%", (progress * 100.0) as usize))
                        },
                        None => (0.0, "Waiting for loop detection...".to_string()),
                    };

                    let main_window_weak = main_window_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        main_window_weak.unwrap().set_progress(progress as f32);
                        main_window_weak.unwrap().set_progress_bar_text(progress_bar_text.into());
                        main_window_weak.unwrap().set_progress_lines(slint_string_arr(status_lines));
                    }).unwrap();
                }
                RenderThreadMessage::RenderComplete => {
                    let main_window_weak = main_window_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        main_window_weak.unwrap().set_rendering(false);
                        main_window_weak.unwrap().set_progress(1.0);
                        main_window_weak.unwrap().set_progress_bar_text("100%".into());
                        main_window_weak.unwrap().set_progress_lines(slint_string_arr(vec![
                            "Done!".to_string()
                        ]));
                    }).unwrap();
                }
            }
        })
    };

    {
        let main_window_weak = main_window.as_weak();
        let mut options = options.clone();
        main_window.on_browse_for_module(move || {
            match browse_for_module_dialog() {
                Some(path) => {
                    let metadata = get_module_metadata(&path);
                    main_window_weak.unwrap().set_module_path(path.clone().into());
                    main_window_weak.unwrap().set_module_metadata(metadata);

                    main_window_weak.unwrap().set_selected_track_index(-1);
                    main_window_weak.unwrap().set_selected_track_text("Select a track...".into());

                    main_window_weak.unwrap().set_track_duration_num("300".into());
                    main_window_weak.unwrap().set_track_duration_type("seconds".into());
                    main_window_weak.unwrap().invoke_update_formatted_duration();

                    options.borrow_mut().input_path = path.into();
                },
                None => ()
            }
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let mut options = options.clone();
        main_window.on_browse_for_background(move || {
            match browse_for_background_dialog() {
                Some(path) => {
                    main_window_weak.unwrap().set_background_path(path.clone().into());

                    options.borrow_mut().video_options.background_path = Some(path.into());
                },
                None => ()
            }
        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let mut options = options.clone();
        main_window.on_update_formatted_duration(move || {
            let module_metadata = main_window_weak.unwrap().get_module_metadata();
            let extended_durations: Vec<i32> = module_metadata.extended_durations
                .iter()
                .collect();
            let selected_track_index = main_window_weak.unwrap().get_selected_track_index();
            let new_duration_type = main_window_weak.unwrap()
                .get_track_duration_type()
                .to_string();
            let new_duration_num = main_window_weak.unwrap()
                .get_track_duration_num()
                .to_string();

            let stop_condition_str = match new_duration_type.as_str() {
                "seconds" => format!("time:{}", new_duration_num),
                "frames" => format!("frames:{}", new_duration_num),
                "loops" => format!("loops:{}", new_duration_num),
                "NSFe/NSF2 duration" => "time:nsfe".to_string(),
                _ => unreachable!()
            };
            if let Ok(stop_condition) = StopCondition::from_str(&stop_condition_str) {
                options.borrow_mut().stop_condition = stop_condition;

                let label = match stop_condition {
                    StopCondition::Frames(frames) => {
                        let seconds = frames as f64 / FRAME_RATE as f64;
                        FormattedDuration(Duration::from_secs_f64(seconds)).to_string()
                    },
                    StopCondition::Loops(_) => "<unknown>".to_string(),
                    StopCondition::NsfeLength => {
                        match extended_durations.get(selected_track_index as usize).cloned() {
                            Some(frames) => {
                                let seconds = frames as f64 / FRAME_RATE as f64;
                                FormattedDuration(Duration::from_secs_f64(seconds)).to_string()
                            },
                            None => "<error>".to_string()
                        }
                    }
                };
                main_window_weak.unwrap().set_track_duration_formatted(label.into());
            }

        });
    }

    {
        let main_window_weak = main_window.as_weak();
        let mut options = options.clone();
        let rt_tx = rt_tx.clone();
        main_window.on_start_render(move || {
            let module_metadata = main_window_weak.unwrap().get_module_metadata();

            let input_path = options.borrow().input_path.clone();
            if input_path.is_empty() || !path::Path::new(&input_path).exists() {
                display_error_dialog("No input file specified.");
                return;
            }
            if !input_path.ends_with(".nsf") {
                display_error_dialog("Input file must have extension '.nsf'.");
                return;
            }

            let output_path = match browse_for_video_dialog() {
                Some(path) => path,
                None => return
            };

            if output_path.ends_with(".mov") && confirm_prores_export_dialog() {
                // Fairly close approximation of the NES' frame rate with a timebase denominator <100000.
                // Required to avoid "codec timebase is very high" warning from the QuickTime encoder.
                options.borrow_mut().video_options.video_time_base = (800, 48_078).into();
                // -c:v prores_ks -profile:v 4 -bits_per_mb 1000 -pix_fmt yuva444p10le
                options.borrow_mut().video_options.video_codec = "prores_ks".to_string();
                options.borrow_mut().video_options.video_codec_params.insert("profile".to_string(), "4".to_string());
                options.borrow_mut().video_options.video_codec_params.insert("bits_per_mb".to_string(), "1000".to_string());
                options.borrow_mut().video_options.pixel_format_out = "yuva444p10le".to_string();
            }

            options.borrow_mut().video_options.output_path = output_path;

            match &options.borrow().stop_condition {
                StopCondition::Loops(_) => {
                    if !module_metadata.loop_detection {
                        display_error_dialog("Loop detection is not supported for this module. Please select a different duration type.");
                        return;
                    }
                },
                StopCondition::NsfeLength => {
                    if module_metadata.extended_durations.iter().len() == 0 {
                        display_error_dialog("This module does not contain extended duration data. Please select a different duration type.");
                        return;
                    }
                },
                _ => ()
            };

            let track_index = match main_window_weak.unwrap().get_selected_track_index() {
                -1 => {
                    display_error_dialog("Please select a track to play.");
                    return;
                },
                index => index as u8 + 1
            };
            options.borrow_mut().track_index = track_index;

            options.borrow_mut().fadeout_length = main_window_weak.unwrap().get_fadeout_duration() as u64;

            let ow = main_window_weak.unwrap().get_output_width() as u32;
            let oh = main_window_weak.unwrap().get_output_height() as u32;
            options.borrow_mut().video_options.resolution_out = (ow, oh);

            options.borrow_mut().famicom = main_window_weak.unwrap().get_famicom_mode();
            options.borrow_mut().high_quality = main_window_weak.unwrap().get_hq_filtering();
            options.borrow_mut().multiplexing = main_window_weak.unwrap().get_multiplexing();

            let mut channel_settings = get_default_channel_settings();
            for ((chip, channel), settings) in channel_settings.iter_mut() {
                let configs_model = match chip.as_str() {
                    "2A03" => main_window_weak.unwrap().get_config_2a03(),
                    "MMC5" => main_window_weak.unwrap().get_config_mmc5(),
                    "N163" => main_window_weak.unwrap().get_config_n163(),
                    "VRC6" => main_window_weak.unwrap().get_config_vrc6(),
                    "VRC7" => main_window_weak.unwrap().get_config_vrc7(),
                    "YM2149F" => main_window_weak.unwrap().get_config_s5b(),
                    "FDS" => main_window_weak.unwrap().get_config_fds(),
                    "APU" => main_window_weak.unwrap().get_config_apu(),
                    _ => continue
                };
                let config = configs_model
                    .as_any()
                    .downcast_ref::<slint::VecModel<ChannelConfig>>()
                    .unwrap()
                    .iter()
                    .find(|cfg| cfg.name.to_string() == channel.clone())
                    .unwrap();

                settings.hidden = config.hidden;
                settings.colors = config.colors
                    .as_any()
                    .downcast_ref::<slint::VecModel<slint::ModelRc<i32>>>()
                    .unwrap()
                    .iter()
                    .map(|color_model| {
                        let mut component_iter = color_model
                            .as_any()
                            .downcast_ref::<slint::VecModel<i32>>()
                            .unwrap()
                            .iter();
                        let r = component_iter.next().unwrap() as u8;
                        let g = component_iter.next().unwrap() as u8;
                        let b = component_iter.next().unwrap() as u8;

                        drawing::Color::rgb(r, g, b)
                    })
                    .collect();
            }
            options.borrow_mut().channel_settings = channel_settings;

            if main_window_weak.unwrap().get_background_path().is_empty() {
                options.borrow_mut().video_options.background_path = None;
            }

            rt_tx.send(Some(options.borrow().clone())).unwrap();
        });
    }

    main_window.run().unwrap();

    if rt_tx.send(None).is_ok() {
        // If the send failed, the channel is closed, so the thread is probably already dead.
        rt_handle.join().unwrap();
    }
}
