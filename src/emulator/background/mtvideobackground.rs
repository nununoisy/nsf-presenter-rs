extern crate ffmpeg_next as ffmpeg;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time;
use rusticnes_ui_common::drawing::{SimpleBuffer, blit, Color};
use ffmpeg::format;
use ffmpeg::software::scaling;
use ffmpeg::util::frame;
use ffmpeg::decoder;
use ffmpeg::media::Type;
use ffmpeg_next::codec;
use ffmpeg_next::format::context::input::PacketIter;
use super::Background;
use crate::video_builder::VideoBuilderUnwrap;

fn spawn_decoding_thread(frames: Arc<Mutex<VecDeque<SimpleBuffer>>>, path: &str, w: u32, h: u32) -> JoinHandle<()> {
    let path = path.to_string();
    thread::spawn(move || {
        println!("[MTVBG] Decoding thread started");

        let mut in_ctx = format::input(&path).unwrap();
        let in_stream = in_ctx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)
            .unwrap();

        let stream_idx = in_stream.index();

        let v_codec_ctx = codec::Context::from_parameters(in_stream.parameters())
            .unwrap();
        let mut v_decoder = v_codec_ctx
            .decoder()
            .video()
            .unwrap();

        let mut sws_ctx = scaling::Context::get(
            v_decoder.format(), v_decoder.width(), v_decoder.height(),
            format::Pixel::RGBA, w, h,
            scaling::Flags::FAST_BILINEAR
        ).unwrap();

        println!("[MTVBG] Starting to decode...");

        let mut decoded_frame = frame::Video::empty();
        let mut rgba_frame = frame::Video::empty();

        for (stream, packet) in in_ctx.packets() {
            if stream.index() == stream_idx {
                v_decoder.send_packet(&packet)
                    .unwrap();

                while v_decoder.receive_frame(&mut decoded_frame).is_ok() {
                    sws_ctx.run(&decoded_frame, &mut rgba_frame)
                        .unwrap();

                    let mut canvas = SimpleBuffer::new(w, h);
                    for (i, color) in rgba_frame.plane::<[u8; 4]>(0).iter().enumerate() {
                        let x = i as u32 % w;
                        let y = i as u32 / w;

                        canvas.put_pixel(x, y, Color::from_slice(color));
                    }

                    {
                        let mut guarded_frames = frames.lock().unwrap();
                        guarded_frames.push_back(canvas);
                        if guarded_frames.len() <= 1800 {
                            continue;
                        }
                    }

                    // Pause decoding if we have too many queued frames and wait for decoder
                    // to consume some before resuming
                    loop {
                        {
                            let guarded_frames = frames.lock().unwrap();
                            if guarded_frames.len() <= 1200 {
                                break;
                            }
                        }
                        thread::sleep(time::Duration::from_millis(100));
                    }
                }
            }
        }

        println!("[MTVBG] Decoding thread stopping");
    })
}

pub struct MTVideoBackground {
    alpha: u8,
    handle: JoinHandle<()>,
    frames: Arc<Mutex<VecDeque<SimpleBuffer>>>,
    frame_idx: usize
}

impl Background for MTVideoBackground {
    fn open(path: &str, w: u32, h: u32, alpha: u8) -> Result<Self, String> {
        let frames: Arc<Mutex<VecDeque<SimpleBuffer>>> = Arc::new(Mutex::new(VecDeque::new()));
        let handle = spawn_decoding_thread(frames.clone(), path, w, h);

        thread::sleep(time::Duration::from_millis(50));

        Ok(Self {
            alpha,
            handle,
            frames,
            frame_idx: 0
        })
    }

    fn step(&mut self, dest: &mut SimpleBuffer) -> Result<(), String> {
        loop {
            let mut guarded_frames = self.frames.lock().unwrap();
            if let Some(frame) = guarded_frames.pop_front() {
                blit(dest, &frame, 0, 0, Color::rgba(255, 255, 255, self.alpha));
                break;
            } else {
                if self.handle.is_finished() {
                    break;
                }
                thread::sleep(time::Duration::from_millis(10));
            }
        };

        Ok(())
    }
}