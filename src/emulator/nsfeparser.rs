use std::collections::HashMap;
use std::collections::vec_deque::VecDeque;
use std::str;
use crate::emulator::NES_NTSC_FRAMERATE;

#[derive(Debug)]
pub enum NsfeChunk {
    Playlist(Vec<usize>),
    Time(Vec<i32>),
    Fadeout(Vec<i32>),
    TrackLabels(Vec<String>),
    TrackAuthors(Vec<String>),
    Author(String, String, String, String),
    Text(String)
}

fn chunk_data_as_i32_vec(chunk_data: &[u8]) -> Vec<i32> {
    chunk_data
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes(c.try_into().expect("bad i32 array")))
        .collect()
}

fn chunk_data_as_string_vec(chunk_data: &[u8]) -> Vec<String> {
    chunk_data
        .into_iter()
        .cloned()
        .fold(Vec::new(), |mut acc, cur| {
            if cur == 0 || acc.is_empty() {
                acc.push(Vec::new());
            }
            if cur != 0 {
                acc.last_mut().unwrap().push(cur);
            }
            acc
        })
        .into_iter()
        .map(|s| str::from_utf8(&s).unwrap().to_string())
        .collect()
}

const DEFAULT_FIELD: &str = "<?>";

fn parse_nsfe_metadata(data: &[u8]) -> Result<Vec<NsfeChunk>, String> {
    let mut data_deque: VecDeque<u8> = VecDeque::from_iter(data.into_iter().cloned());
    let mut result: Vec<NsfeChunk> = Vec::new();

    println!("Parsing NSFe chunks:");
    while !data_deque.is_empty() {
        let chunk_len = u32::from_le_bytes(data_deque.drain(0..4).collect::<Vec<_>>().try_into().unwrap());
        let four_cc = data_deque.drain(0..4).collect::<Vec<_>>().try_into().unwrap();
        let chunk_data: Vec<u8> = data_deque.drain(0..chunk_len as usize).collect();

        let chunk = match &four_cc {
            b"plst" => {
                let playlist: Vec<usize> = chunk_data
                    .into_iter()
                    .map(|t| 1 + t as usize)
                    .collect();
                NsfeChunk::Playlist(playlist)
            },
            b"time" => NsfeChunk::Time(chunk_data_as_i32_vec(&chunk_data)),
            b"fade" => NsfeChunk::Fadeout(chunk_data_as_i32_vec(&chunk_data)),
            b"tlbl" => NsfeChunk::TrackLabels(chunk_data_as_string_vec(&chunk_data)),
            b"taut" => NsfeChunk::TrackAuthors(chunk_data_as_string_vec(&chunk_data)),
            b"auth" => {
                let strings = chunk_data_as_string_vec(&chunk_data);

                let title = strings.get(0).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let artist = strings.get(1).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let copyright = strings.get(2).unwrap_or(&DEFAULT_FIELD.to_string()).clone();
                let ripper = strings.get(3).unwrap_or(&DEFAULT_FIELD.to_string()).clone();

                NsfeChunk::Author(title, artist, copyright, ripper)
            },
            b"text" => NsfeChunk::Text(chunk_data_as_string_vec(&chunk_data).get(0).unwrap_or(&DEFAULT_FIELD.to_string()).clone()),
            b"NEND" => break,
            unk_four_cc => {
                println!("  {} {} <?>", str::from_utf8(unk_four_cc).unwrap(), chunk_len);
                continue;
            }
        };

        println!("  {} {} {:?}", str::from_utf8(&four_cc).unwrap(), chunk_len, chunk);

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
    text: Option<String>
}

macro_rules! track {
    ($metadata: ident, $idx: expr) => {
        *$metadata.tracks.entry($idx).or_insert(NsfeTrack::new())
    };
}

impl NsfeMetadata {
    pub fn from(data: &[u8]) -> Result<Self, String> {
        let mut metadata = Self {
            chunks: parse_nsfe_metadata(data)?,
            tracks: HashMap::new(),
            playlist: None,
            title: None,
            artist: None,
            copyright: None,
            ripper: None,
            text: None
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
                NsfeChunk::Author(title, artist, copyright, ripper) => {
                    metadata.title = Some(title.to_owned());
                    metadata.artist = Some(artist.to_owned());
                    metadata.copyright = Some(copyright.to_owned());
                    metadata.ripper = Some(ripper.to_owned());
                }
                NsfeChunk::Text(text) => {
                    metadata.text = Some(text.to_owned());
                }
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
}
