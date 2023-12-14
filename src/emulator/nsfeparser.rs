use anyhow::{Result, ensure, bail, Context};
use std::collections::HashMap;
use std::collections::vec_deque::VecDeque;
use std::str;
use std::mem;
use crate::emulator::NES_NTSC_FRAMERATE;

#[derive(Clone, Debug)]
pub enum NsfeChunk {
    Playlist(Vec<usize>),
    SoundEffects(Vec<usize>),
    Time(Vec<i32>),
    Fadeout(Vec<i32>),
    TrackLabels(Vec<String>),
    TrackAuthors(Vec<String>),
    Author { title: String, artist: String, copyright: String, ripper: String },
    Text(String),
    Info(Vec<u8>),
    Data(Vec<u8>),
    BankInit(Vec<u8>),
    NSF2Flags(u8),
    Rate(Vec<u16>),
    VRC7 { use_ym2413: bool, patches: Option<[u8; 8 * 15]>, rhythm_patches: Option<[u8; 8 * 3]> }
}

fn chunk_data_as_u16_vec(chunk_data: &[u8]) -> Result<Vec<u16>> {
    ensure!((chunk_data.len() % mem::size_of::<u16>()) == 0, "NSFe u16 array has invalid length");

    Ok(chunk_data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes(c.try_into().unwrap()))  // error should never occur
        .collect())
}

fn chunk_data_as_i32_vec(chunk_data: &[u8]) -> Result<Vec<i32>> {
    ensure!((chunk_data.len() % mem::size_of::<i32>()) == 0, "NSFe i32 array has invalid length");

    Ok(chunk_data
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))  // error should never occur
        .collect())
}

fn chunk_data_as_string_vec(chunk_data: &[u8]) -> Result<Vec<String>> {
    // chunk_data
    //     .into_iter()
    //     .cloned()
    //     .fold(Vec::new(), |mut acc, cur| {
    //         if cur == 0 || acc.is_empty() {
    //             acc.push(Vec::new());
    //         }
    //         if cur != 0 {
    //             acc.last_mut().unwrap().push(cur);
    //         }
    //         acc
    //     })
    //     .into_iter()
    //     .map(|s| str::from_utf8(&s)?.to_string())
    //     .collect()

    chunk_data
        .split(|&b| b == 0)
        .map(|s| Ok(str::from_utf8(s).context("NSFe string array contains invalid data")?.to_string()))
        .collect()
}

const DEFAULT_FIELD: &str = "<?>";

fn extract_fourcc_chunks(data: &[u8]) -> Result<Vec<([u8; 4], Vec<u8>)>> {
    let mut data_deque: VecDeque<u8> = VecDeque::from_iter(data.into_iter().cloned());
    let mut result: Vec<([u8; 4], Vec<u8>)> = Vec::new();

    while !data_deque.is_empty() {
        ensure!(data_deque.len() >= 8, "Not enough data left for next NSFe chunk!");

        let chunk_len = u32::from_le_bytes(data_deque.drain(0..4).collect::<Vec<_>>().try_into().unwrap()) as usize;
        let four_cc = data_deque.drain(0..4).collect::<Vec<_>>().try_into().unwrap();
        let chunk_data: Vec<u8> = data_deque.drain(0..chunk_len).collect();

        ensure!(chunk_data.len() == chunk_len, "NSFe chunk is too short");

        result.push((four_cc, chunk_data));
    }

    Ok(result)
}

fn parse_nsfe_metadata(data: &[u8]) -> Result<Vec<NsfeChunk>> {
    let mut result: Vec<NsfeChunk> = Vec::new();

    for (four_cc, chunk_data) in extract_fourcc_chunks(data)? {
        let chunk = match &four_cc {
            b"plst" => {
                let playlist: Vec<usize> = chunk_data
                    .into_iter()
                    .map(|t| 1 + t as usize)
                    .collect();
                NsfeChunk::Playlist(playlist)
            },
            b"psfx" => {
                let sound_effects: Vec<usize> = chunk_data
                    .into_iter()
                    .map(|t| 1 + t as usize)
                    .collect();
                NsfeChunk::SoundEffects(sound_effects)
            },
            b"time" => NsfeChunk::Time(chunk_data_as_i32_vec(&chunk_data)?),
            b"fade" => NsfeChunk::Fadeout(chunk_data_as_i32_vec(&chunk_data)?),
            b"tlbl" => NsfeChunk::TrackLabels(chunk_data_as_string_vec(&chunk_data)?),
            b"taut" => NsfeChunk::TrackAuthors(chunk_data_as_string_vec(&chunk_data)?),
            b"auth" => {
                let strings = chunk_data_as_string_vec(&chunk_data)?;

                let title = strings.get(0).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let artist = strings.get(1).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let copyright = strings.get(2).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let ripper = strings.get(3).unwrap_or(&DEFAULT_FIELD.to_string()).clone();

                NsfeChunk::Author { title, artist, copyright, ripper }
            },
            b"text" => NsfeChunk::Text(chunk_data_as_string_vec(&chunk_data)?.get(0).unwrap_or(&DEFAULT_FIELD.to_string()).clone()),
            b"INFO" => NsfeChunk::Info(chunk_data),
            b"DATA" => NsfeChunk::Data(chunk_data),
            b"BANK" => NsfeChunk::BankInit(chunk_data),
            b"NSF2" => NsfeChunk::NSF2Flags(chunk_data.get(0).cloned().unwrap_or_default()),
            b"RATE" => NsfeChunk::Rate(chunk_data_as_u16_vec(&chunk_data)?),
            b"VRC7" => {
                let use_ym2413 = (chunk_data.get(0).cloned().context("VRC7 section missing YM2413 flag")?) != 0;
                let (patches, rhythm_patches) = match (use_ym2413, chunk_data.len()) {
                    (_, 1) => (None, None),
                    (_, 129) => (Some(chunk_data[9..129].try_into()?), None),
                    (true, 153) => (Some(chunk_data[9..129].try_into()?), Some(chunk_data[129..153].try_into()?)),
                    (false, 153) => bail!("VRC7 section specifies rhythm instruments in non-YM2413 mode"),
                    _ => bail!("VRC7 section has invalid length {}", chunk_data.len())
                };

                NsfeChunk::VRC7 { use_ym2413, patches, rhythm_patches }
            }
            b"NEND" => break,
            unk_four_cc => {
                println!("Warning: unknown fourcc {}", str::from_utf8(unk_four_cc).unwrap());
                continue;
            }
        };

        result.push(chunk);
    }

    Ok(result)
}

#[derive(Clone)]
pub struct NsfeTrack {
    pub label: Option<String>,
    pub author: Option<String>,
    pub duration: Option<usize>,
    pub fadeout: Option<usize>
}

impl NsfeTrack {
    pub fn new() -> Self {
        Self {
            label: None,
            author: None,
            duration: None,
            fadeout: None
        }
    }
}

pub struct NsfeMetadata {
    chunks: Vec<NsfeChunk>,
    tracks: HashMap<usize, NsfeTrack>,
    playlist: Option<Vec<usize>>,
    title: Option<String>,
    artist: Option<String>,
    copyright: Option<String>,
    ripper: Option<String>,
    text: Option<String>,
    vrc7_patches: Option<[u8; 8 * 15]>
}

macro_rules! track {
    ($metadata: ident, $idx: expr) => {
        *$metadata.tracks.entry($idx).or_insert(NsfeTrack::new())
    };
}

impl NsfeMetadata {
    pub fn from(data: &[u8]) -> Result<Self> {
        let mut metadata = Self {
            chunks: parse_nsfe_metadata(data)?,
            tracks: HashMap::new(),
            playlist: None,
            title: None,
            artist: None,
            copyright: None,
            ripper: None,
            text: None,
            vrc7_patches: None
        };

        for chunk in &metadata.chunks {
            match chunk {
                NsfeChunk::Playlist(playlist) => metadata.playlist = Some(playlist.to_owned()),
                NsfeChunk::Time(times) => {
                    for (i, t) in times.iter().enumerate() {
                        track!(metadata, i+1).duration = Some((t.clone() as f64 * NES_NTSC_FRAMERATE / 1000.0) as usize);
                    }
                },
                NsfeChunk::Fadeout(fadeouts) => {
                    for (i, f) in fadeouts.iter().enumerate() {
                        track!(metadata, i+1).fadeout = Some((f.clone() as f64 * NES_NTSC_FRAMERATE / 1000.0) as usize);
                    }
                },
                NsfeChunk::TrackLabels(labels) => {
                    for (i, l) in labels.iter().enumerate() {
                        track!(metadata, i+1).label = Some(l.clone());
                    }
                },
                NsfeChunk::TrackAuthors(authors) => {
                    for (i, a) in authors.iter().enumerate() {
                        track!(metadata, i+1).author = Some(a.clone());
                    }
                },
                NsfeChunk::Author { title, artist, copyright, ripper } => {
                    metadata.title = Some(title.to_owned());
                    metadata.artist = Some(artist.to_owned());
                    metadata.copyright = Some(copyright.to_owned());
                    metadata.ripper = Some(ripper.to_owned());
                }
                NsfeChunk::Text(text) => {
                    metadata.text = Some(text.to_owned());
                },
                NsfeChunk::VRC7 { use_ym2413, patches, .. } => {
                    metadata.vrc7_patches = patches.to_owned();
                    if *use_ym2413 {
                        println!("Warning: YM2413 mode currently not supported");
                    }
                }
                _ => ()
            }
        };

        Ok(metadata)
    }

    pub fn track(&self, index: usize) -> Option<NsfeTrack> {
        self.tracks.get(&index).cloned()
    }

    pub fn title(&self) -> Option<String> {
        self.title.clone()
    }

    pub fn artist(&self) -> Option<String> {
        self.artist.clone()
    }

    pub fn copyright(&self) -> Option<String> {
        self.copyright.clone()
    }

    pub fn ripper(&self) -> Option<String> {
        self.ripper.clone()
    }

    pub fn track_title(&self, index: usize) -> Option<String> {
        self.track(index)?.label
    }

    pub fn track_author(&self, index: usize) -> Option<String> {
        self.track(index)?.author
    }

    pub fn track_duration(&self, index: usize) -> Option<usize> {
        self.track(index)?.duration
    }

    pub fn track_fadeout(&self, index: usize) -> Option<usize> {
        self.track(index)?.fadeout
    }

    pub fn vrc7_patches(&self) -> Option<[u8; 8 * 15]> {
        self.vrc7_patches.clone()
    }
}

pub fn nsfe_to_nsf2(data: &[u8]) -> Result<Vec<u8>> {
    ensure!(&data[0..4] == b"NSFE", "Malformed header");

    let mut result: Vec<u8> = Vec::new();
    let chunks = extract_fourcc_chunks(&data[4..])?;
    let parsed_chunks = parse_nsfe_metadata(&data[4..])?;

    let info = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::Info(i) => Some(i.clone()),
        _ => None
    }).context("Missing INFO chunk")?;

    let rom_data = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::Data(i) => Some(i.clone()),
        _ => None
    }).context("Missing DATA chunk")?;

    let mut bank_init = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::BankInit(i) => Some(i.clone()),
        _ => None
    }).unwrap_or_default();
    bank_init.resize(8, 0);
    
    let (title, artist, copyright) = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::Author { title, artist, copyright, .. } => Some((
            title.clone(),
            artist.clone(),
            copyright.clone()
        )),
        _ => None
    }).unwrap_or((DEFAULT_FIELD.to_string(), DEFAULT_FIELD.to_string(), DEFAULT_FIELD.to_string()));

    let rates = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::Rate(i) => Some(i.clone()),
        _ => None
    }).unwrap_or_default();

    let nsf2_flags = parsed_chunks.iter().find_map(|c| match c {
        NsfeChunk::NSF2Flags(i) => Some(i.clone()),
        _ => None
    }).unwrap_or_default();

    result.extend_from_slice(b"NESM\x1A");  // NSF magic
    result.push(2);  // version
    result.push(info[8]);  // total songs
    result.push(info.get(9).cloned().unwrap_or_default() + 1);  // starting song
    result.extend_from_slice(&info[0..6]);  // load/init/play addresses

    macro_rules! formatted_metadata_field {
        ($f: tt) => {{
            let mut b = $f.into_bytes();
            b.truncate(0x1F);
            b.resize(0x20, 0);
            b
        }};
    }
    result.extend(formatted_metadata_field!(title));  // module metadata
    result.extend(formatted_metadata_field!(artist));
    result.extend(formatted_metadata_field!(copyright));

    result.extend_from_slice(&rates.get(0).cloned().unwrap_or(16_639).to_le_bytes());  // NTSC rate
    result.extend(bank_init);  // bankswitch init values
    result.extend_from_slice(&rates.get(1).cloned().unwrap_or(19_997).to_le_bytes());  // PAL rate
    result.push(info[6]);  // PAL/NTSC bits
    result.push(info[7]);  // Expansion audio bits
    result.push(nsf2_flags | 0x80);  // NSF2 flags
    result.extend_from_slice(&(rom_data.len() as u32).to_le_bytes()[0..3]);  // NSF2 program length

    result.extend(rom_data);  // ROM data

    // NSF2 extended metadata
    for (four_cc, chunk_data) in chunks {
        // Ignore NSFe specific chunks
        match &four_cc {
            b"INFO" => continue,
            b"DATA" => continue,
            b"BANK" => continue,
            b"NSF2" => continue,
            _ => ()
        }

        result.extend_from_slice(&(chunk_data.len() as u32).to_le_bytes());
        result.extend_from_slice(&four_cc);
        result.extend(chunk_data);
    }

    Ok(result)
}
