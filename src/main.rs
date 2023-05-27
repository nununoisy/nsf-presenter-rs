mod video_builder;
mod emulator;
mod renderer;
mod cli;
mod gui_old;
mod gui;

use std::env;
use build_time::build_time_utc;

fn main() {
    println!("NSFPresenter started! (built {})", build_time_utc!("%Y-%m-%dT%H:%M:%S"));
    video_builder::init().unwrap();

    match env::args().len() {
        1 => gui::run(),
        _ => cli::run()
    };
}
