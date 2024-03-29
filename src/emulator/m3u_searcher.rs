use anyhow::{Result, Context};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use glob::{glob_with, MatchOptions};
use encoding_rs::{CoderResult, WINDOWS_1252, SHIFT_JIS};

fn read_m3u_file<P: AsRef<Path>>(m3u_path: P) -> Result<String> {
    let data = fs::read(m3u_path)?;
    let mut result = String::with_capacity(data.len() * 4);

    let mut cp1252_decoder = WINDOWS_1252.new_decoder();
    let (coder_result, _bytes_read, did_replacements) = cp1252_decoder.decode_to_string(&data, &mut result, true);
    if coder_result != CoderResult::OutputFull && !did_replacements {
        return Ok(result);
    }

    result.clear();
    let mut shift_jis_decoder = SHIFT_JIS.new_decoder();
    let (coder_result, _bytes_read, did_replacements) = shift_jis_decoder.decode_to_string(&data, &mut result, true);
    if coder_result != CoderResult::OutputFull && !did_replacements {
        return Ok(result);
    }

    String::from_utf8(data).context("M3U string is not valid CP-1252, Shift-JIS, or UTF-8")
}

pub fn search<P: AsRef<Path>>(nsf_path: P) -> Result<HashMap<u8, (String, Option<Duration>)>> {
    let mut result: HashMap<u8, (String, Option<Duration>)> = HashMap::new();

    let nsf_filename = nsf_path.as_ref().file_name().unwrap().to_str().unwrap().to_string();

    let mut nsf_dir = nsf_path
        .as_ref()
        .parent()
        .context("Invalid path")?
        .canonicalize()?;
    nsf_dir.push("*.m3u");

    let mut nsf_dir = nsf_dir.to_str().unwrap().to_string();
    if nsf_dir.starts_with("\\\\?\\") {
        let _ = nsf_dir.drain(0..4);
    }

    let options = MatchOptions {
        case_sensitive: false,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    for glob_entry in glob_with(&nsf_dir, options)? {
        let m3u_path = glob_entry?;
        println!("Discovered M3U file: {}", m3u_path.file_name().unwrap().to_str().unwrap());

        for line in read_m3u_file(m3u_path)?.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut components: Vec<String> = Vec::new();
            for raw_component in line.split(',') {
                if !components.is_empty() && components.last().unwrap().replace("\\\\", "").ends_with('\\') {
                    let _ = components.last_mut().unwrap().pop();
                    components.last_mut().unwrap().push(',');
                    components.last_mut().unwrap().push_str(&raw_component.replace("\\\\", "\\"));
                } else {
                    components.push(raw_component.replace("\\\\", "\\"));
                }
            }
            let mut component_iter = components.iter().cloned();

            let filename = component_iter.next().unwrap_or("".to_string());
            if filename.to_lowercase() != format!("{}::nsf", nsf_filename.to_lowercase()) {
                continue;
            }

            let index = u8::from_str(&component_iter.next().unwrap_or("".to_string()))
                .context("M3U track index is missing/invalid")?
                .saturating_sub(1);

            let mut track_title = component_iter.next().unwrap_or("".to_string());
            if track_title.is_empty() {
                continue;
            } else if track_title.chars().count() > 60 {
                let new_len = track_title.char_indices().nth(57).map(|(i, _)| i).unwrap_or(track_title.len());
                track_title.truncate(new_len);
                track_title.push_str("...");
            }

            let duration_seconds = component_iter.next().unwrap_or("".to_string())
                .split(':')
                .fold(0.0_f64, |acc, cur| {
                    let duration_component = f64::from_str(cur).unwrap_or_default();
                    (acc * 60.0) + duration_component
                });
            let duration = if duration_seconds > 0.0 {
                Some(Duration::from_secs_f64(duration_seconds))
            } else {
                None
            };

            result.insert(index, (track_title, duration));
        }
    }

    Ok(result)
}