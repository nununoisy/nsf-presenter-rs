use std::thread;
use std::sync::mpsc;
use std::str::FromStr;
use std::path::PathBuf;
use std::sync::mpsc::RecvError;
use crate::renderer::{Renderer, options::{RendererOptions, StopCondition, FRAME_RATE}};
use crate::video_builder::SampleFormat;
use crate::emulator::Emulator;
use indicatif::{FormattedDuration, HumanBytes};
use std::time::{Duration, Instant};
use fltk::{app, prelude::*};
use fltk::window::Window;
use fltk::button::{Button, CheckButton};
use fltk::input::IntInput;
use fltk::frame::Frame;
use fltk::dialog::{FileDialog, FileDialogType, alert_default, message_default};
use fltk::group::Flex;
use fltk::enums::{Align, Color, FrameType};
use fltk::misc::{InputChoice, Progress, Tooltip};
use fltk_theme::{WidgetScheme, SchemeType};
use fltk_theme::widget_schemes::aqua::frames::*;
use fltk_theme::colors::aqua::dark::*;

#[derive(Clone)]
enum RendererRequest {
    ChangeInputPath(String),
    ChangeOutputPath(String),
    ChangeSampleRate(i32),
    ChangeTrackIndex(u8),
    ChangeStopCondition(StopCondition),
    ChangeFadeoutLength(Option<u64>),
    ChangeOutSize(u32, u32),
    ChangeFamicom(bool),
    ChangeHQ(bool),
    ChangeMuxing(bool),
    StartRender
}

#[derive(Clone)]
enum RendererResponse {
    Error(String),
    InputMetadata(String, String, String),
    RenderStarting,
    RenderProgress(u64, u32, i64, Option<usize>, Option<Duration>, Option<Duration>, Duration, Duration, String),
    RenderComplete
}

fn renderer_thread() -> (thread::JoinHandle<()>, mpsc::Sender<RendererRequest>, mpsc::Receiver<RendererResponse>) {
    let (req_tx, req_rx) = mpsc::channel::<RendererRequest>();
    let (res_tx, res_rx) = mpsc::channel::<RendererResponse>();
    let join_handle = thread::spawn(move || {
        let mut options = RendererOptions {
            input_path: "".to_string(),
            output_path: "".to_string(),
            v_codec: "libx264".to_string(),
            a_codec: "aac".to_string(),
            pix_fmt: "yuv420p".to_string(),
            sample_fmt: SampleFormat::FLTP,
            sample_rate: 44100,
            track_index: 1,
            stop_condition: StopCondition::Loops(2),
            fadeout_length: 180,
            ow: 1920,
            oh: 1080,
            famicom: false,
            high_quality: false,
            multiplexing: false,
            v_codec_opts: None,
            a_codec_opts: None,
        };

        let mut tmp_emulator = Emulator::new();
        tmp_emulator.init(None);

        loop {
            match req_rx.recv().unwrap() {
                RendererRequest::ChangeInputPath(p) => {
                    options.input_path = p.clone();
                    if tmp_emulator.open(&p).is_ok() {
                        match tmp_emulator.nsf_metadata() {
                            Ok(Some((title, artist, copyright))) => {
                                res_tx.send(RendererResponse::InputMetadata(title, artist, copyright)).unwrap()
                            },
                            _ => ()
                        }
                    }
                },
                RendererRequest::ChangeOutputPath(p) => options.output_path = p.clone(),
                RendererRequest::ChangeSampleRate(r) => options.sample_rate = r,
                RendererRequest::ChangeTrackIndex(i) => {
                    println!("CHANGE TRACK INDEX: {} {}", i, tmp_emulator.track_count());
                    if i > 0 && i <= tmp_emulator.track_count() {
                        options.track_index = i;
                        tmp_emulator.select_track(i);
                        match tmp_emulator.nsf_metadata() {
                            Ok(Some((title, artist, copyright))) => {
                                res_tx.send(RendererResponse::InputMetadata(title, artist, copyright)).unwrap()
                            },
                            _ => ()
                        }
                    }
                },
                RendererRequest::ChangeStopCondition(c) => options.stop_condition = c,
                RendererRequest::ChangeFadeoutLength(Some(l)) => options.fadeout_length = l,
                RendererRequest::ChangeFadeoutLength(None) => {
                    match tmp_emulator.nsfe_duration() {
                        Some(d) => options.fadeout_length = d as u64,
                        None => res_tx.send(RendererResponse::Error("Track does not have NSFe/NSF2 duration".to_string())).unwrap()
                    }
                }
                RendererRequest::ChangeOutSize(w, h) => {
                    options.ow = w;
                    options.oh = h;
                }
                RendererRequest::ChangeFamicom(f) => options.famicom = f,
                RendererRequest::ChangeHQ(h) => options.high_quality = h,
                RendererRequest::ChangeMuxing(m) => options.multiplexing = m,
                RendererRequest::StartRender => {
                    if options.input_path.is_empty() {
                        res_tx.send(RendererResponse::Error("No input file specified".to_string())).unwrap();
                        continue;
                    }

                    if !options.input_path.ends_with(".nsf") {
                        res_tx.send(RendererResponse::Error("Input file is not an NSF".to_string())).unwrap();
                        continue;
                    }

                    if options.output_path.is_empty() {
                        res_tx.send(RendererResponse::Error("No output file specified".to_string())).unwrap();
                        continue;
                    }

                    if !options.output_path.ends_with(".mp4") {
                        res_tx.send(RendererResponse::Error("Output file is not an MP4".to_string())).unwrap();
                        continue;
                    }

                    match &options.stop_condition {
                        StopCondition::Loops(_) => {
                            if tmp_emulator.loop_count().is_none() {
                                res_tx.send(RendererResponse::Error("Loop detection not supported for this NSF".to_string())).unwrap();
                                continue;
                            }
                        },
                        StopCondition::NsfeLength => {
                            if tmp_emulator.nsfe_duration().is_none() {
                                res_tx.send(RendererResponse::Error("NSF does not have a specified duration".to_string())).unwrap();
                                continue;
                            }
                        },
                        _ => ()
                    }

                    break;
                }
            }
        }

        res_tx.send(RendererResponse::RenderStarting).unwrap();

        let mut renderer = Renderer::new(options).unwrap();
        renderer.start_encoding().unwrap();

        let mut last_progress_timestamp = Instant::now();
        // Janky way to force an update
        last_progress_timestamp.checked_sub(Duration::from_secs(2));

        loop {
            if !renderer.step().unwrap() {
                break;
            }

            if last_progress_timestamp.elapsed().as_secs_f64() >= 0.5 {
                last_progress_timestamp = Instant::now();
                res_tx.send(RendererResponse::RenderProgress(
                    renderer.cur_frame(),
                    renderer.average_fps(),
                    renderer.encoded_size(),
                    renderer.expected_duration_frames(),
                    renderer.expected_duration(),
                    renderer.eta_duration(),
                    renderer.elapsed(),
                    renderer.encoded_duration(),
                    renderer.emulator_progress().unwrap()
                )).unwrap();
            }
        }

        renderer.finish_encoding().unwrap();
        res_tx.send(RendererResponse::RenderComplete).unwrap();
    });

    (join_handle, req_tx, res_rx)
}

macro_rules! apply_scheme_color {
    ($color: ident, $ac: tt) => {{
        let c = $color.to_rgb();
        app::$ac(c.0, c.1, c.2);
    }};
    (INP $i: tt) => {{
        $i.set_color(*controlColor);
    }};
    (BTN $b: tt) => {{
        $b.set_color(*controlColor);
        $b.set_selection_color(*controlAccentColor);
        $b.set_frame(OS_DEFAULT_BUTTON_UP_BOX);
    }};
    (CHK $c: tt) => {{
        $c.set_frame(FrameType::FlatBox);
    }};
    (ICH $i: tt) => {{
        $i.input().set_color(*controlColor);
        $i.menu_button().set_color(*controlColor);
        $i.menu_button().set_selection_color(*controlAccentColor);
        $i.set_frame(OS_DEFAULT_BUTTON_UP_BOX);
    }};
    (PRG $p: tt) => {{
        $p.set_color(*controlColor);
        $p.set_selection_color(*controlAccentColor);
    }};
}

pub fn run() {
    let (render_thread, req_tx, res_rx) = renderer_thread();

    let app = app::App::default()
        .with_scheme(app::Scheme::Gtk);

    apply_scheme_color!(windowBackgroundColor, background);
    apply_scheme_color!(controlAccentColor, background2);
    apply_scheme_color!(labelColor, foreground);
    app::set_color(Color::Selection, 255, 255, 255);

    let scheme = WidgetScheme::new(SchemeType::Aqua);
    scheme.apply();

    let mut opt_win = Window::default()
        .with_size(600, 440)
        .with_label("NSFPresenter");

    let mut main_flx = Flex::default_fill().column();

    let mut title_lbl = Frame::default().with_label("NSFPresenter");
    main_flx.set_size(&title_lbl, 30);

    let mut metadata_lbl = Frame::default().with_label("<no song loaded>");
    main_flx.set_size(&metadata_lbl, 30);

    let mut input_file_flx = Flex::default().row();
    let mut input_file_lbl = Frame::default()
        .with_label("Input: <none>")
        .with_align(Align::Right | Align::Inside);
    let mut input_file_btn = Button::default().with_label("Browse...");
    apply_scheme_color!(BTN input_file_btn);
    let mut input_file_dlg = FileDialog::new(FileDialogType::BrowseFile);
    input_file_dlg.set_filter("Nintendo Sound Files\t*.nsf");
    {
        let mut req_tx = req_tx.clone();
        input_file_btn.set_callback(move |_| {
            input_file_dlg.show();
            let filename = input_file_dlg
                .filename()
                .to_str()
                .unwrap_or("")
                .to_string();
            if !filename.is_empty() {
                let basename = input_file_dlg
                    .filename()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                input_file_lbl.set_label(format!("Input: {}", basename).as_str());
                req_tx.send(RendererRequest::ChangeInputPath(filename.to_string())).unwrap();
            }
        });
    }
    input_file_flx.end();
    main_flx.set_size(&input_file_flx, 30);

    let mut output_file_flx = Flex::default().row();
    let mut output_file_lbl = Frame::default()
        .with_label("Output: <none>")
        .with_align(Align::Right | Align::Inside);
    let mut output_file_btn = Button::default().with_label("Browse...");
    apply_scheme_color!(BTN output_file_btn);
    let mut output_file_dlg = FileDialog::new(FileDialogType::BrowseSaveFile);
    output_file_dlg.set_filter("MPEG-4 Video\t*.mp4");
    {
        let mut req_tx = req_tx.clone();
        output_file_btn.set_callback(move |_| {
            output_file_dlg.show();
            let filename = output_file_dlg
                .filename()
                .to_str()
                .unwrap_or("")
                .to_string();
            if !filename.is_empty() {
                let basename = output_file_dlg
                    .filename()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                output_file_lbl.set_label(format!("Output: {}", basename).as_str());
                req_tx.send(RendererRequest::ChangeOutputPath(filename.to_string())).unwrap();
            }
        });
    }
    output_file_flx.end();
    main_flx.set_size(&output_file_flx, 30);

    let mut nsf_track_flx = Flex::default().row();
    let mut nsf_track_lbl = Frame::default()
        .with_label("NSF track:")
        .with_align(Align::Right | Align::Inside);
    let mut nsf_track_inp = IntInput::default();
    apply_scheme_color!(INP nsf_track_inp);
    nsf_track_inp.set_value("1");
    {
        let mut req_tx = req_tx.clone();
        nsf_track_inp.set_callback(move |inp| {
            if let Ok(track_index) = u8::from_str(&inp.value()) {
                req_tx.send(RendererRequest::ChangeTrackIndex(track_index)).unwrap();
            }
        });
    }
    nsf_track_flx.end();
    main_flx.set_size(&nsf_track_flx, 30);

    let mut stop_cond_flx = Flex::default().row();
    let mut stop_cond_lbl = Frame::default()
        .with_label("Stop at: 2 loops")
        .with_align(Align::Right | Align::Inside);
    let mut stop_cond_ich = InputChoice::default();
    apply_scheme_color!(ICH stop_cond_ich);
    stop_cond_ich.add("loops:2");
    stop_cond_ich.add("time:nsfe");
    stop_cond_ich.add("time:300");
    stop_cond_ich.add("frames:18000");
    stop_cond_ich.set_value_index(0);
    {
        let mut req_tx = req_tx.clone();
        stop_cond_ich.set_callback(move |ich| {
            if let Some(value) = ich.value() {
                if let Ok(stop_condition) = StopCondition::from_str(&value) {
                    req_tx.send(RendererRequest::ChangeStopCondition(stop_condition.clone())).unwrap();

                    let label = match stop_condition {
                        StopCondition::Duration(frames) => {
                            let seconds = frames as f64 / FRAME_RATE as f64;
                            let duration = FormattedDuration(Duration::from_secs_f64(seconds));
                            format!("Stop at: {} ({}f)", duration, frames)
                        },
                        StopCondition::Loops(loops) => format!("Stop at: {} loops", loops),
                        StopCondition::NsfeLength => "Stop at: NSFe/NSF2 duration".to_string()
                    };
                    stop_cond_lbl.set_label(&label);
                }
            }
        });
    }
    stop_cond_flx.end();
    main_flx.set_size(&stop_cond_flx, 30);

    let mut fadeout_len_flx = Flex::default().row();
    Frame::default()
        .with_label("Fadeout length (frames):")
        .with_align(Align::Right | Align::Inside);
    let mut fadeout_len_inp = IntInput::default();
    apply_scheme_color!(INP fadeout_len_inp);
    fadeout_len_inp.set_value("180");
    {
        let mut req_tx = req_tx.clone();
        fadeout_len_inp.set_callback(move |inp| {
            if let Ok(fadeout_length) = u64::from_str(&inp.value()) {
                req_tx.send(RendererRequest::ChangeFadeoutLength(Some(fadeout_length))).unwrap();
            }
        });
    }
    fadeout_len_flx.end();
    main_flx.set_size(&fadeout_len_flx, 30);

    let mut out_size_flx = Flex::default().row();
    let mut out_size_lbl = Frame::default()
        .with_label("Output size:")
        .with_align(Align::Right | Align::Inside);
    let mut out_size_ich = InputChoice::default();
    apply_scheme_color!(ICH out_size_ich);
    out_size_ich.add("1920x1080");
    out_size_ich.add("3840x2160");
    out_size_ich.set_value_index(0);
    {
        let mut req_tx = req_tx.clone();
        out_size_ich.set_callback(move |ich| {
            if let Some(value) = ich.value() {
                // remove all whitespace
                let value_no_ws: String = value
                    .split_whitespace()
                    .collect();
                let components: Vec<String> = value_no_ws
                    .to_lowercase()
                    .split("x")
                    .map(|s| s.to_string())
                    .collect();
                if components.len() == 2 {
                    match (u32::from_str(&components[0]), u32::from_str(&components[1])) {
                        (Ok(w), Ok(h)) => {
                            req_tx.send(RendererRequest::ChangeOutSize(w, h)).unwrap();
                        },
                        _ => ()
                    }
                }
            }
        });
    }
    out_size_flx.end();
    main_flx.set_size(&out_size_flx, 30);

    let mut famicom_chk = CheckButton::default()
        .with_label("Famicom mode");
    apply_scheme_color!(CHK famicom_chk);
    famicom_chk.set_tooltip("Emulate the Famicom's filter chain instead of the NES'. The output \
                             will be noisier but render speed improves slightly.");
    {
        let mut req_tx = req_tx.clone();
        famicom_chk.set_callback(move |chk| {
            req_tx.send(RendererRequest::ChangeFamicom(chk.value())).unwrap();
        });
    }
    main_flx.set_size(&famicom_chk, 30);

    let mut high_quality_chk = CheckButton::default()
        .with_label("High-quality filtering");
    apply_scheme_color!(CHK high_quality_chk);
    high_quality_chk.set_tooltip("Use high-quality filter emulation. Can make rendering slow.");
    {
        let mut req_tx = req_tx.clone();
        high_quality_chk.set_callback(move |chk| {
            req_tx.send(RendererRequest::ChangeHQ(chk.value())).unwrap();
        });
    }
    main_flx.set_size(&high_quality_chk, 30);

    let mut multiplexing_chk = CheckButton::default()
        .with_label("Emulate multiplexing");
    apply_scheme_color!(CHK multiplexing_chk);
    multiplexing_chk.set_tooltip("Emulate multiplexing on mappers that mix with it (e.g. N163). \
                                  It causes popping artifacts which may be undesirable, but some \
                                  songs intentionally use this for effects.");
    {
        let mut req_tx = req_tx.clone();
        multiplexing_chk.set_callback(move |chk| {
            req_tx.send(RendererRequest::ChangeMuxing(chk.value())).unwrap();
        });
    }
    main_flx.set_size(&multiplexing_chk, 30);

    let mut start_render_btn = Button::default()
        .with_label("Start Render");
    apply_scheme_color!(BTN start_render_btn);
    {
        let mut req_tx = req_tx.clone();
        start_render_btn.set_callback(move |_| {
            req_tx.send(RendererRequest::StartRender).unwrap();
        });
    }
    main_flx.set_size(&start_render_btn, 30);

    main_flx.end();

    opt_win.end();

    let mut progress_win = Window::default()
        .with_size(600, 200)
        .with_label("NSFPresenter");

    let mut p_main_flx = Flex::default_fill().column();

    let mut p_metadata_lbl = Frame::default()
        .with_label("<no song loaded>");
    p_main_flx.set_size(&p_metadata_lbl, 30);

    let mut p_status_lbl = Frame::default()
        .with_label("Starting render...");
    p_main_flx.set_size(&p_status_lbl, 30);

    let mut p_emu_status_lbl = Frame::default()
        .with_label("");
    p_main_flx.set_size(&p_emu_status_lbl, 30);

    let mut p_progress_bar = Progress::default()
        .with_label("");
    apply_scheme_color!(PRG p_progress_bar);
    p_main_flx.set_size(&p_progress_bar, 30);

    p_main_flx.end();

    progress_win.end();

    opt_win.show();

    let gui_update_thread = thread::spawn(move || {
        loop {
            if let Ok(res) = res_rx.try_recv() {
                app::lock().unwrap();
                match res {
                    RendererResponse::Error(msg) => {
                        app::awake_callback(move || {
                            alert_default(&msg);
                        });
                    },
                    RendererResponse::InputMetadata(title, artist, copyright) => {
                        let metadata_text = format!("{} - {} - {}", title, artist, copyright);
                        metadata_lbl.set_label(metadata_text.as_str());
                        p_metadata_lbl.set_label(metadata_text.as_str());
                        app::awake();
                    },
                    RendererResponse::RenderStarting => {
                        let mut opt_win = opt_win.clone();
                        let mut progress_win = progress_win.clone();
                        app::awake_callback(move || {
                            opt_win.hide();
                            progress_win.show();
                        });
                    },
                    RendererResponse::RenderProgress(cur_frame, avg_fps, enc_size,
                                                     exp_dur_frames, exp_dur,
                                                     eta_dur, ela_dur, enc_dur,
                                                     emu_progress) => {
                        let current_video_size = HumanBytes(enc_size as u64);
                        let current_video_duration = FormattedDuration(enc_dur);
                        let expected_video_duration = match exp_dur {
                            Some(duration) => FormattedDuration(duration).to_string(),
                            None => "?".to_string()
                        };
                        let elapsed_duration = FormattedDuration(ela_dur);
                        let eta_duration = match eta_dur {
                            Some(duration) => FormattedDuration(duration).to_string(),
                            None => "?".to_string()
                        };
                        let status = format!(
                            "{} FPS, frame: {} | Encoded {}/{}, size: {}",
                            avg_fps, cur_frame,
                            current_video_duration, expected_video_duration, current_video_size
                        );
                        let emu_status = format!(
                            "{} | Elapsed: {}/{}",
                            emu_progress, elapsed_duration, eta_duration
                        );
                        p_status_lbl.set_label(status.as_str());
                        p_emu_status_lbl.set_label(emu_status.as_str());

                        match exp_dur_frames {
                            Some(dur_frames) => {
                                p_progress_bar.set_maximum(dur_frames as f64);
                                p_progress_bar.set_value(cur_frame as f64);
                                p_progress_bar.set_label(format!("{}%", cur_frame * 100 / dur_frames as u64).as_str())
                            },
                            None => {
                                p_progress_bar.set_maximum(1.0);
                                p_progress_bar.set_value(0.0);
                                p_progress_bar.set_label("Waiting for loop detection...");
                            }
                        }
                        app::awake();
                    },
                    RendererResponse::RenderComplete => {
                        println!("Done!");
                        p_progress_bar.set_maximum(1.0);
                        p_progress_bar.set_value(1.0);
                        p_progress_bar.set_label("100%");
                        let mut opt_win = opt_win.clone();
                        let mut progress_win = progress_win.clone();
                        app::awake_callback(move || {
                            message_default("Done!");
                            opt_win.hide();
                            progress_win.hide();
                        });
                        app::unlock();
                        break;
                    }
                }
                app::unlock();
            }
        }
    });

    app.run().unwrap();

    // Terminate threads immediately
    std::process::exit(0);
}
