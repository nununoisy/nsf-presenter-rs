extern crate ffmpeg_next as ffmpeg;

use std::collections::VecDeque;
use std::ffi::CStr;
use std::str::FromStr;
use std::mem::ManuallyDrop;
use ffmpeg_next::{Error, format, Rational, Dictionary, encoder, ChannelLayout, frame, Packet, Codec, StreamMut};
use ffmpeg_next::codec::{self, Id, context::Context};
use ffmpeg_next::format::sample::Type::{Packed, Planar};
use ffmpeg_next::software;
use ffmpeg_sys_next::{av_version_info, avcodec_alloc_context3, avcodec_parameters_from_context, avcodec_parameters_to_context, avutil_configuration, FF_PROFILE_AAC_LOW};

pub fn init() -> Result<(), Error> {
    ffmpeg::init()
}

pub fn print_ffmpeg_info() {
    unsafe {
        println!("FFMPEG info: version {}, config: {}", CStr::from_ptr(av_version_info()).to_str().unwrap(),
                 CStr::from_ptr(avutil_configuration()).to_str().unwrap());
    }
}

pub trait VideoBuilderUnwrap<T> {
    fn vb_unwrap(self) -> Result<T, String>;
}

impl<T> VideoBuilderUnwrap<T> for Result<T, Error> {
    fn vb_unwrap(self) -> Result<T, String> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("FFMPEG error: {}", e))
        }
    }
}

impl<T> VideoBuilderUnwrap<T> for Result<T, format::pixel::ParsePixelError> {
    fn vb_unwrap(self) -> Result<T, String> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("FFMPEG pixel parsing error: {}", e))
        }
    }
}

impl<T> VideoBuilderUnwrap<T> for Option<T> {
    fn vb_unwrap(self) -> Result<T, String> {
        match self {
            Some(v) => Ok(v),
            None => Err("Not fully initialized".to_string())
        }
    }
}

fn ffmpeg_create_context(codec: Codec, parameters: codec::Parameters) -> Result<Context, String> {
    // ffmpeg-next does not provide a way to pass a codec to avcodec_alloc_context3, which
    // is necessary for initializing certain contexts (e.g. mp4/libx264).
    // Safety: The return value of avcodec_alloc_context3() is checked to ensure that the allocation
    //         succeeded.
    // Safety: The allocated context is wrapped in a safe abstraction, which handles freeing the
    //         associated resources later.
    // Safety: The value of avcodec_parameters_to_context is checked to ensure errors are handled.
    unsafe {
        let context = avcodec_alloc_context3(codec.as_ptr());
        if context.is_null() {
            return Err("FFMPEG error: avcodec_alloc_context3() failed".to_string());
        }

        let mut context = Context::wrap(context, None);
        match avcodec_parameters_to_context(context.as_mut_ptr(), parameters.as_ptr()) {
            0 => Ok(context),
            e => Err(Error::from(e).to_string())
        }
    }
}

fn ffmpeg_copy_codec_params(stream: &mut StreamMut, context: &Context, codec: &Codec) -> Result<(), String> {
    // This context copy is required to initialize some codecs. ffmpeg-next does not
    // provide a safe abstraction so it must be done here.
    // Safety: The value of avcodec_parameters_from_context is checked to ensure errors are handled.
    // Safety: All mutable pointer dereferences are done strictly on initialized memory since they
    //         come from a mutable reference to a safe abstraction.
    unsafe {
        match avcodec_parameters_from_context((*stream.as_mut_ptr()).codecpar, context.as_ptr()) {
            0 => (),
            e => return Err(Error::from(e).to_string())
        };
        (*(*stream.as_mut_ptr()).codecpar).codec_id = codec.id().into();
        (*(*stream.as_mut_ptr()).codecpar).codec_type = codec.medium().into();
        // (*(*stream.as_mut_ptr()).codec).time_base = stream.time_base().into();
    }
    Ok(())
}

// fn ffmpeg_copy_context_params() {}

fn ffmpeg_rational_to_f64(rational: &Rational) -> f64 {
    let num = rational.numerator() as f64;
    let den = rational.denominator() as f64;
    num / den
}

fn ffmpeg_context_bytes_written(context: &format::context::Output) -> i64 {
    unsafe {
        (*(*context.as_ptr()).pb).written
    }
}

#[derive(Copy, Clone)]
pub enum SampleFormat {
    U8,
    S16,
    S32,
    S64,
    F32,
    F64,
    U8P,
    S16P,
    S32P,
    S64P,
    FLTP,
    DBLP
}

pub struct VideoBuilder {
    out_ctx: format::context::Output,
    v_time_base: Option<Rational>,
    a_time_base: Option<Rational>,
    v_encoder: Option<encoder::Video>,
    a_encoder: Option<encoder::Audio>,
    v_swc_ctx: Option<software::scaling::Context>,
    v_sws_ctx: Option<software::scaling::Context>,
    a_sw_ctx: Option<software::resampling::Context>,
    v_stream_idx: usize,
    a_stream_idx: usize,
    v_pts: i64,
    a_pts: i64,
    v_mux_pts: i64,
    a_mux_pts: i64,
    v_frame_buf: VecDeque<Vec<u8>>,
    a_frame_buf: VecDeque<i16>,
    v_in_frame: Option<frame::Video>,
    v_resize_frame: Option<frame::Video>,
    v_out_frame: frame::Video,
    a_in_frame: Option<frame::Audio>,
    a_out_frame: frame::Audio
}

impl VideoBuilder {
    pub fn new(path: &str) -> Result<Self, String> {
        Ok(Self {
            out_ctx: format::output(&path).vb_unwrap()?,
            v_time_base: None,
            a_time_base: None,
            v_encoder: None,
            a_encoder: None,
            v_swc_ctx: None,
            v_sws_ctx: None,
            a_sw_ctx: None,
            v_stream_idx: 0,
            a_stream_idx: 0,
            v_pts: 0,
            a_pts: 0,
            v_mux_pts: 0,
            a_mux_pts: 0,
            v_frame_buf: VecDeque::new(),
            a_frame_buf: VecDeque::new(),
            v_in_frame: None,
            v_resize_frame: None,
            v_out_frame: frame::Video::empty(),
            a_in_frame: None,
            a_out_frame: frame::Audio::empty()
        })
    }

    pub fn set_metadata(&mut self, title: &str, composer: &str, copyright: &str,
                        artist: Option<&str>, filename: Option<&str>, track: Option<(u32, u32)>,
                        comment: Option<&str>) {
        let mut metadata = Dictionary::new();

        metadata.set("title", title);
        metadata.set("composer", composer);
        metadata.set("copyright", copyright);
        metadata.set("album", copyright);

        if let Some(artist) = artist {
            metadata.set("artist", artist);
        } else {
            metadata.set("artist", composer);
        }

        if let Some(filename) = filename {
            metadata.set("filename", filename);
        }
        if let Some((track_num, track_count)) = track {
            metadata.set("track", format!("{}/{}", track_num, track_count).as_str());
        }
        if let Some(comment) = comment {
            metadata.set("comment", comment);
        } else {
            metadata.set("comment", format!("Encoded with NSFPresenter").as_str())
        }

        self.out_ctx.set_metadata(metadata);
    }

    pub fn set_video_params<I>(&mut self, iw: u32, ih: u32, ow: u32, oh: u32, fps: i32,
                               pix_fmt: &str, codec_name: &str, codec_options: Option<I>) -> Result<(), String>
    where
        I: IntoIterator<Item = (String, String)>
    {
        if self.v_encoder.is_some() {
            return Err("Video params already set for this builder".to_string());
        }

        if fps <= 0 {
            return Err("FPS must be a positive integer".to_string());
        }

        let time_base = match fps {
            60 => Rational::new(29_781, 1_789_773), // Close to correct NES framerate
            f => Rational::new(1, f)
        };

        let output_format = format::Pixel::from_str(pix_fmt)
            .vb_unwrap()?;

        let codec = encoder::find_by_name(codec_name)
            .ok_or_else(|| format!("Unknown codec {}", codec_name))?;
        let mut stream = self.out_ctx.add_stream(codec).vb_unwrap()?;

        let mut context = ffmpeg_create_context(codec, stream.parameters())?
            .encoder()
            .video()
            .vb_unwrap()?;

        context.set_format(output_format);
        context.set_width(ow);
        context.set_height(oh);
        context.set_max_b_frames(2);
        context.set_gop(12);
        context.set_time_base(time_base);
        // context.set_bit_rate(25_000_000);

        ffmpeg_copy_codec_params(&mut stream, &context, &codec)?;

        stream.set_time_base(time_base);

        let mut context_options = Dictionary::new();
        match codec.id() {
            Id::H264 | Id::H265 => {
                context_options.set("preset", "veryfast");
                context_options.set("crf", "16");
                context_options.set("tune", "animation");
            },
            _ => ()
        }
        if let Some(opts) = codec_options {
            for (k, v) in opts {
                context_options.set(&k, &v);
            }
        }

        let v_encoder = context.open_as_with(codec, context_options)
            .vb_unwrap()?;

        self.v_time_base = Some(time_base);
        self.v_encoder = Some(v_encoder);
        self.v_swc_ctx = Some(
            software::converter((iw, ih), format::Pixel::RGBA, output_format)
                .vb_unwrap()?
        );
        if iw != ow || ih != oh {
            self.v_sws_ctx = Some(
                software::scaler(output_format, software::scaling::Flags::POINT, (iw, ih), (ow, oh))
                    .vb_unwrap()?
            );
        }
        self.v_stream_idx = stream.index();
        self.v_pts = 0;
        self.v_mux_pts = 0;

        Ok(())
    }

    pub fn set_audio_params<I>(&mut self, sample_rate: i32, sample_fmt: SampleFormat, codec_name: &str, codec_options: Option<I>) -> Result<(), String>
    where
        I: IntoIterator<Item = (String, String)>
    {
        if self.a_encoder.is_some() {
            return Err("Audio params already set for this builder".to_string());
        }

        if sample_rate <= 0 {
            return Err("Rate must be a positive integer".to_string());
        }

        let time_base = Rational::new(1, sample_rate);

        let sample_format = match sample_fmt {
            SampleFormat::U8 => format::Sample::U8(Packed),
            SampleFormat::S16 => format::Sample::I16(Packed),
            SampleFormat::S32 => format::Sample::I32(Packed),
            SampleFormat::S64 => format::Sample::I64(Packed),
            SampleFormat::F32 => format::Sample::F32(Packed),
            SampleFormat::F64 => format::Sample::F64(Packed),
            SampleFormat::U8P => format::Sample::U8(Planar),
            SampleFormat::S16P => format::Sample::I16(Planar),
            SampleFormat::S32P => format::Sample::I32(Planar),
            SampleFormat::S64P => format::Sample::I64(Planar),
            SampleFormat::FLTP => format::Sample::F32(Planar),
            SampleFormat::DBLP => format::Sample::F64(Planar),
        };

        let codec = encoder::find_by_name(codec_name)
            .ok_or_else(|| format!("Unknown codec {}", codec_name))?;
        let mut stream = self.out_ctx.add_stream(codec).vb_unwrap()?;

        let mut context = ffmpeg_create_context(codec, stream.parameters())?
            .encoder()
            .audio()
            .vb_unwrap()?;

        context.set_rate(sample_rate);
        context.set_format(sample_format);
        context.set_channels(1);
        context.set_channel_layout(ChannelLayout::MONO);
        context.set_time_base(time_base);
        context.set_bit_rate(192_000);

        ffmpeg_copy_codec_params(&mut stream, &context, &codec)?;

        stream.set_time_base(time_base);

        // TODO make this less hacky
        unsafe {
            (*(*stream.as_mut_ptr()).codecpar).frame_size = 1024;
            // (*(*stream.as_mut_ptr()).codecpar).profile = FF_PROFILE_AAC_LOW;
            // (*context.as_mut_ptr()).profile = FF_PROFILE_AAC_LOW;
        }

        let mut context_options = Dictionary::new();
        context_options.set("profile", "aac_low");
        context_options.set("profile:a", "aac_low");
        if let Some(opts) = codec_options {
            for (k, v) in opts {
                context_options.set(&k, &v);
            }
        }

        let a_encoder = context.open_as_with(codec, context_options)
            .vb_unwrap()?;

        // Ugly hack to ensure AAC profile propagates to output file
        // Without this, audio does not work on Windows Media Player
        // TODO isolate into function, safety checks
        unsafe {
            avcodec_parameters_from_context((*stream.as_mut_ptr()).codecpar, a_encoder.as_ptr());
        }

        self.a_time_base = Some(time_base);
        self.a_encoder = Some(a_encoder);
        self.a_sw_ctx = Some(
            software::resampler(
                (format::Sample::I16(Planar), ChannelLayout::MONO, sample_rate as _),
                (sample_format, ChannelLayout::MONO, sample_rate as _)
            )
                .vb_unwrap()?
        );
        self.a_stream_idx = stream.index();
        self.a_pts = 0;
        self.a_mux_pts = 0;

        Ok(())
    }

    pub fn push_video_data(&mut self, video: Vec<u8>) -> Result<(), String> {
        self.v_frame_buf.push_back(video);
        Ok(())
    }

    pub fn push_audio_data(&mut self, audio: Vec<i16>) -> Result<(), String> {
        self.a_frame_buf.extend(audio);
        Ok(())
    }

    fn planarize_video_data(video: Vec<u8>) -> Result<Vec<(u8, u8, u8, u8)>, String> {
        if video.len() % 4 != 0 {
            return Err(format!("Video data size not divisible by plane count"));
        }

        let planar_data = match std::mem::size_of::<(u8, u8, u8, u8)>() {
            // This unsafe Vec construct allows us to avoid an unneeded copy on supported platforms.
            // This copy occurs every frame so optimizing it out boosts FPS significantly.
            // Safety: The tuple's byte length is checked before transmutation to ensure the memory
            //         layout is sane. If this is violated, fall back to a safe copy instead.
            // Safety: The size of the input Vec is checked before transmutation to ensure no tuple
            //         element points to uninitialized memory. If this is violated, return an Err.
            4 => unsafe {
                let mut v_clone = ManuallyDrop::new(video);
                Vec::from_raw_parts(v_clone.as_mut_ptr() as *mut (u8, u8, u8, u8), v_clone.len() / 4, v_clone.capacity() / 4)
            },
            _ => video.chunks_exact(4).map(|c| (c[0], c[1], c[2], c[3])).collect()
        };
        Ok(planar_data)
    }

    fn send_video_to_encoder(&mut self, video: Vec<u8>) -> Result<(), String> {
        let encoder = self.v_encoder.as_mut().ok_or_else(|| "Video not initialized")?;
        let swc_ctx = self.v_swc_ctx.as_mut().ok_or_else(|| "Video not initialized")?;

        if self.v_in_frame.is_none() {
            let new_frame = frame::Video::new(swc_ctx.input().format, swc_ctx.input().width, swc_ctx.input().height);
            self.v_in_frame = Some(new_frame);
        }
        let input_frame = self.v_in_frame.as_mut().vb_unwrap()?;

        let plane_data: Vec<(u8, u8, u8, u8)> = Self::planarize_video_data(video)?;
        input_frame.plane_mut::<(u8, u8, u8, u8)>(0).copy_from_slice(plane_data.as_slice());

        let output_frame = &mut self.v_out_frame;

        if let Some(sws_ctx) = self.v_sws_ctx.as_mut() {
            if self.v_resize_frame.is_none() {
                let new_frame = frame::Video::new(swc_ctx.output().format, swc_ctx.output().width, swc_ctx.output().height);
                self.v_resize_frame = Some(new_frame);
            }
            let resize_frame = self.v_resize_frame.as_mut().vb_unwrap()?;

            swc_ctx.run(input_frame, resize_frame).vb_unwrap()?;
            sws_ctx.run(resize_frame, output_frame).vb_unwrap()?;
        } else {
            swc_ctx.run(input_frame, output_frame).vb_unwrap()?;
        }

        output_frame.set_pts(Some(self.v_pts));
        encoder.send_frame(output_frame).vb_unwrap()?;

        self.v_pts += 1;

        Ok(())
    }

    fn send_audio_to_encoder(&mut self, audio: Vec<i16>) -> Result<(), String> {
        let encoder = self.a_encoder.as_mut().ok_or_else(|| "Audio not initialized")?;
        let sw_ctx = self.a_sw_ctx.as_mut().ok_or_else(|| "Audio not initialized")?;

        if self.a_in_frame.is_none() {
            let new_frame = frame::Audio::new(sw_ctx.input().format, audio.len(), sw_ctx.input().channel_layout);
            self.a_in_frame = Some(new_frame);
        }
        let input_frame = self.a_in_frame.as_mut().vb_unwrap()?;
        input_frame.set_rate(sw_ctx.input().rate);

        input_frame.plane_mut::<i16>(0).copy_from_slice(&audio);

        let output_wave = &mut self.a_out_frame;

        sw_ctx.run(input_frame, output_wave).vb_unwrap()?;

        output_wave.set_pts(Some(self.a_pts));
        encoder.send_frame(output_wave).vb_unwrap()?;

        self.a_pts += audio.len() as i64;

        Ok(())
    }

    fn mux_video_frame(&mut self, packet: &mut Packet) -> Result<bool, String> {
        let v_time_base = self.v_time_base.ok_or_else(|| "Video not initialized")?;
        let v_encoder = self.v_encoder.as_mut().ok_or_else(|| "Video not initialized")?;

        if v_encoder.receive_packet(packet).is_ok() {
            let out_time_base = self.out_ctx
                .stream(self.v_stream_idx)
                .vb_unwrap()?
                .time_base();

            packet.rescale_ts(v_time_base, out_time_base);
            packet.set_stream(self.v_stream_idx);

            packet.write_interleaved(&mut self.out_ctx).vb_unwrap()?;

            self.v_mux_pts += 1;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn mux_audio_frame(&mut self, packet: &mut Packet) -> Result<bool, String> {
        let a_time_base = self.a_time_base.ok_or_else(|| "Audio not initialized")?;
        let a_encoder = self.a_encoder.as_mut().ok_or_else(|| "Audio not initialized")?;

        if a_encoder.receive_packet(packet).is_ok() {
            let out_time_base = self.out_ctx
                .stream(self.a_stream_idx)
                .vb_unwrap()?
                .time_base();

            packet.rescale_ts(a_time_base, out_time_base);
            packet.set_stream(self.a_stream_idx);

            packet.write_interleaved(&mut self.out_ctx).vb_unwrap()?;

            self.a_mux_pts += 1;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn step_encoding(&mut self) -> Result<(), String> {
        let mut packet = Packet::empty();

        loop {
            if self.a_mux_pts <= self.v_mux_pts && self.a_frame_buf.len() >= 1024 {
                let audio = self.a_frame_buf.drain(0..1024).collect();
                self.send_audio_to_encoder(audio)?;
                if !(self.mux_audio_frame(&mut packet)?) {
                    break;
                }
            } else if let Some(video) = self.v_frame_buf.pop_front() {
                self.send_video_to_encoder(video)?;
                if !(self.mux_video_frame(&mut packet)?) {
                    break;
                }
            } else {
                break;
            }
        };

        Ok(())
    }

    pub fn start_encoding(&mut self) -> Result<(), String> {
        println!("Starting encoding:");
        // TODO find a way to make this work on FFmpeg 5.0
        // for stream in self.out_ctx.streams() {
        //     println!("  #{} {:?} {} {}", stream.index(), stream.codec().medium(),
        //              stream.codec().id().name(), stream.codec().codec().vb_unwrap()?.description());
        // }

        let mut opts = Dictionary::new();
        match self.out_ctx.format().name() {
            "mp4" => opts.set("movflags", "faststart"),
            _ => ()
        };

        self.out_ctx.write_header_with(opts).vb_unwrap()?;
        Ok(())
    }

    pub fn finish_encoding(&mut self) -> Result<(), String> {
        let v_encoder = self.v_encoder.as_mut().ok_or_else(|| "Video not initialized")?;
        let a_encoder = self.a_encoder.as_mut().ok_or_else(|| "Audio not initialized")?;

        v_encoder.send_eof().vb_unwrap()?;
        a_encoder.send_eof().vb_unwrap()?;

        let mut packet = Packet::empty();
        loop {
            let muxed_audio = self.mux_audio_frame(&mut packet)?;
            let muxed_video = self.mux_video_frame(&mut packet)?;

            if !muxed_audio && !muxed_video {
                break;
            }
        }

        self.out_ctx.write_trailer().vb_unwrap()
    }

    pub fn encoded_video_duration(&self) -> Result<f64, String> {
        let v_time_base = self.v_time_base.as_ref().ok_or_else(|| "Video not initialized")?;
        Ok(self.v_pts as f64 * ffmpeg_rational_to_f64(v_time_base))
    }

    pub fn encoded_video_bytes(&self) -> i64 {
        ffmpeg_context_bytes_written(&self.out_ctx)
    }
}
