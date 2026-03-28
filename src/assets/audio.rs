use std::{
    num::{NonZeroU32, NonZeroUsize},
    ops::Range,
};

use bevy::{asset::AssetLoader, prelude::*, tasks::futures_lite::AsyncRead};
use bevy_seedling::{
    context::{SampleRate, StreamStartEvent},
    firewheel::sample_resource::{SampleResource, SampleResourceInfo},
    sample::AudioSample,
};
use ffmpeg_async_utils::input_from_reader;
use ffmpeg_next as ffmpeg;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.preregister_asset_loader::<AudioLoader>(AudioLoader::extensions())
            .add_observer(on_stream_start);
    }
}

fn on_stream_start(
    _: On<StreamStartEvent>,
    sample_rate: Res<SampleRate>,
    asset_server: Res<AssetServer>,
) {
    asset_server.register_loader(AudioLoader {
        sample_rate: sample_rate.clone(),
    });
}

#[derive(TypePath)]
struct AudioLoader {
    sample_rate: SampleRate,
}

impl AudioLoader {
    /// Any extension. The decoder can probe the format.
    const fn extensions() -> &'static [&'static str] {
        &[]
    }
}

impl AssetLoader for AudioLoader {
    type Asset = AudioSample;
    type Settings = ();
    type Error = ffmpeg::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut reader = reader as &mut (dyn AsyncRead + Unpin);
        let mut input = input_from_reader(&mut reader)?;

        let input_stream = input
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let stream_index = input_stream.index();

        let codec_context =
            ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
        let mut decoder = codec_context.decoder().audio()?;

        let target_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar);
        let target_sample_rate = self.sample_rate.get();

        let mut resampler = ffmpeg::software::resampling::context::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            target_format,
            decoder.channel_layout(),
            target_sample_rate.get(),
        )?;

        let mut samples = vec![Vec::new(); decoder.channels() as usize];

        let mut decoded_frame = ffmpeg::frame::Audio::empty();
        let mut resampled_frame = ffmpeg::frame::Audio::empty();

        let mut process_resampled_frame = |frame: &ffmpeg::frame::Audio| {
            for (i, channel) in samples.iter_mut().enumerate() {
                let plane = frame.plane::<f32>(i);
                channel.extend_from_slice(plane);
            }
        };

        let mut process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Audio| -> Result<(), ffmpeg::Error> {
                loop {
                    match decoder.receive_frame(&mut decoded_frame) {
                        Ok(()) => (),
                        Err(ffmpeg::Error::Other {
                            errno: ffmpeg::ffi::EAGAIN,
                        })
                        | Err(ffmpeg::Error::Eof) => return Ok(()),
                        Err(e) => return Err(e),
                    }

                    resampler.run(&decoded_frame, &mut resampled_frame)?;

                    while resampled_frame.samples() > 0 {
                        process_resampled_frame(&resampled_frame);
                        resampler.flush(&mut resampled_frame)?;
                    }
                }
            };

        for (stream, packet) in input.packets() {
            if stream.index() == stream_index {
                decoder.send_packet(&packet)?;
                process_decoded_frames(&mut decoder)?;
            }
        }
        // Flush the decoder
        decoder.send_eof()?;
        process_decoded_frames(&mut decoder)?;

        Ok(AudioSample::new(
            DecodedAudio {
                samples,
                sample_rate: target_sample_rate,
            },
            decoder.rate().try_into().unwrap(),
        ))
    }

    fn extensions(&self) -> &[&str] {
        Self::extensions()
    }
}

struct DecodedAudio {
    samples: Vec<Vec<f32>>,
    sample_rate: NonZeroU32,
}

impl SampleResourceInfo for DecodedAudio {
    fn num_channels(&self) -> NonZeroUsize {
        self.samples.num_channels()
    }

    fn len_frames(&self) -> u64 {
        self.samples.len_frames()
    }

    fn sample_rate(&self) -> Option<NonZeroU32> {
        Some(self.sample_rate)
    }
}

impl SampleResource for DecodedAudio {
    fn fill_buffers(
        &self,
        buffers: &mut [&mut [f32]],
        buffer_range: Range<usize>,
        start_frame: u64,
    ) {
        self.samples
            .fill_buffers(buffers, buffer_range, start_frame);
    }
}
