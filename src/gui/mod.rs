use slint;
use rfd::FileDialog;
use std::rc::Rc;
use std::fs;
use crate::emulator::{Nsf, NsfDriverType};

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
    result.chips = slint_string_arr(chips);
    result.tracks = slint_string_arr(tracks);

    result
}

fn browse_for_module() -> Option<String> {
    let file = FileDialog::new()
        .add_filter("Nintendo Sound Files", &["nsf"])
        .pick_file();

    match file {
        Some(path) => Some(path.to_str().unwrap().to_string()),
        None => None
    }
}

pub fn run() {
    let main_window = MainWindow::new().unwrap();

    {
        let main_window_weak = main_window.as_weak();
        main_window.on_browse_for_module(move || {
            match browse_for_module() {
                Some(path) => {
                    let metadata = get_module_metadata(&path);
                    main_window_weak.unwrap().set_module_path(path.into());
                    main_window_weak.unwrap().set_module_metadata(metadata);
                },
                None => ()
            }
        });
    }

    main_window.run().unwrap();
}
