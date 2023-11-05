use std::ffi::CString;
use ffmpeg_next::{codec, Codec, Error, format, StreamMut};
use ffmpeg_sys_next::{av_get_sample_fmt, avcodec_alloc_context3, avcodec_parameters_from_context, avcodec_parameters_to_context};

pub fn ffmpeg_create_context(codec: Codec, parameters: codec::Parameters) -> Result<codec::Context, String> {
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

        let mut context = codec::Context::wrap(context, None);
        match avcodec_parameters_to_context(context.as_mut_ptr(), parameters.as_ptr()) {
            0 => Ok(context),
            e => Err(Error::from(e).to_string())
        }
    }
}

pub fn ffmpeg_copy_context_params(stream: &mut StreamMut, context: &codec::Context) -> Result<(), String> {
    // This context copy is required to fully initialize some codecs (e.g. AAC). ffmpeg-next does not
    // provide a safe abstraction so it must be done here.
    // Safety: The value of avcodec_parameters_from_context is checked to ensure errors are handled.
    // Safety: All mutable pointer dereferences are done strictly on initialized memory since they
    //         come from a mutable reference to a safe abstraction.
    unsafe {
        match avcodec_parameters_from_context((*stream.as_mut_ptr()).codecpar, context.as_ptr()) {
            0 => Ok(()),
            e => Err(Error::from(e).to_string())
        }
    }
}

pub fn ffmpeg_copy_codec_params(stream: &mut StreamMut, context: &codec::Context, codec: &Codec) -> Result<(), String> {
    // This augmented context copy is required to initialize some codecs. ffmpeg-next does not
    // provide a safe abstraction so it must be done here.
    // Safety: All mutable pointer dereferences are done strictly on initialized memory since they
    //         come from a mutable reference to a safe abstraction.
    unsafe {
        ffmpeg_copy_context_params(stream, context)?;
        (*(*stream.as_mut_ptr()).codecpar).codec_id = codec.id().into();
        (*(*stream.as_mut_ptr()).codecpar).codec_type = codec.medium().into();
    }
    Ok(())
}

pub fn ffmpeg_sample_format_from_string(value: &str) -> format::Sample {
    // This is provided by ffmpeg-next, but only for `&'static str`, presumably due to
    // some confusion over the `const char*` in the method signature?
    unsafe {
        let value = CString::new(value).unwrap();

        format::Sample::from(av_get_sample_fmt(value.as_ptr()))
    }
}

pub fn ffmpeg_get_audio_context_frame_size(context: &codec::Context, variable_frame_size: usize) -> usize {
    let frame_size = unsafe { (*context.as_ptr()).frame_size as usize };
    let ctx_codec = context.codec().unwrap();
    debug_assert!(ctx_codec.is_audio());
    debug_assert!(ctx_codec.is_encoder());

    if ctx_codec.capabilities().contains(codec::Capabilities::VARIABLE_FRAME_SIZE) || frame_size == 0 {
        variable_frame_size
    } else {
        frame_size
    }
}

pub fn ffmpeg_context_bytes_written(context: &format::context::Output) -> usize {
    #[cfg(not(feature = "ffmpeg_6_0"))]
    let bytes_written = unsafe { (*(*context.as_ptr()).pb).written };
    #[cfg(feature = "ffmpeg_6_0")]
    let bytes_written = unsafe { (*(*context.as_ptr()).pb).bytes_written };
    std::cmp::max(bytes_written, 0) as usize
}