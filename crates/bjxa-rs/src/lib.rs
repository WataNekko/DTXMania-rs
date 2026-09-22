#![allow(clippy::chunks_exact_to_as_chunks)]

use std::{
    io::{self, Read},
    iter::{self, FlatMap},
    num::{NonZero, NonZeroU8, NonZeroU16, NonZeroUsize},
};

const HEADER_LEN: usize = 32;
const HEADER_MAGIC: u32 = u32::from_le_bytes(*b"KWD1");
const SAMPLES_PER_BLOCK: usize = 32;
const GAIN_FACTOR: [(i32, i32); 5] = [(0, 0), (240, 0), (460, -208), (392, -220), (488, -240)];

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidHeader,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(io) => io,
            e => Self::other(e),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Format {
    pub samples_per_channel: NonZeroUsize,
    pub sample_rate: NonZeroU16,
    pub channels: NonZeroU8,
}

enum BitDepth {
    Four = 4,
    Six = 6,
    Eight = 8,
}

impl TryFrom<u8> for BitDepth {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            4 => Ok(Self::Four),
            6 => Ok(Self::Six),
            8 => Ok(Self::Eight),
            _ => Err(()),
        }
    }
}

struct PrevSamples(i16, i16);

pub struct DecodedSample {
    channel: usize,
    sample: i16,
}

pub struct Decoder<R: Read> {
    pub reader: R,
    fmt: Format,
    block_len: usize,
    bits: BitDepth,
    channels_state: [PrevSamples; 2],
}

type InflatedIter<'a> =
    FlatMap<std::slice::ChunksExact<'a, u8>, Vec<i16>, fn(&'a [u8]) -> Vec<i16>>;

impl<R: Read> Decoder<R> {
    pub fn new(mut reader: R) -> Result<Decoder<R>, Error> {
        let mut header = [0u8; HEADER_LEN];
        reader.read_exact(&mut header)?;

        // Extract fields
        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let data_len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let samples_per_channel = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let sample_rate = u16::from_le_bytes(header[12..14].try_into().unwrap());
        let bits = header[14];
        let channels = header[15];
        let channels_state = [
            PrevSamples(
                i16::from_le_bytes(header[20..22].try_into().unwrap()),
                i16::from_le_bytes(header[22..24].try_into().unwrap()),
            ),
            PrevSamples(
                i16::from_le_bytes(header[24..26].try_into().unwrap()),
                i16::from_le_bytes(header[26..28].try_into().unwrap()),
            ),
        ];

        let block_len = (bits as usize) * SAMPLES_PER_BLOCK / 8 + 1;
        // total samples length per block + profile byte

        match (
            magic,
            NonZero::new(data_len),
            NonZero::new(samples_per_channel),
            NonZero::new(sample_rate),
            BitDepth::try_from(bits),
            channels,
        ) {
            (
                HEADER_MAGIC,
                Some(data_len),
                Some(samples_per_channel),
                Some(sample_rate),
                Ok(bits),
                1 | 2,
            ) if data_len.get() % block_len == 0 && {
                let blocks = data_len.get() / block_len;
                let blocks_per_channel = blocks / (channels as usize);

                let upper = blocks_per_channel * SAMPLES_PER_BLOCK;
                let lower = upper - SAMPLES_PER_BLOCK + 1;
                (lower..=upper).contains(&samples_per_channel.get())
            } =>
            {
                Ok(Self {
                    reader,
                    fmt: Format {
                        samples_per_channel,
                        sample_rate,
                        channels: channels.try_into().unwrap(),
                    },
                    block_len,
                    bits,
                    channels_state,
                })
            }
            _ => Err(Error::InvalidHeader),
        }
    }

    pub fn format(&mut self) -> Format {
        self.fmt
    }

    fn inflate_4bit_samples(bytes: &mut [u8]) -> InflatedIter<'_> {
        bytes.chunks_exact(1).flat_map(|b| {
            let b = b[0] as u16;
            vec![((b & 0xf0) << 8) as i16, ((b & 0x0f) << 12) as i16]
        })
    }

    fn inflate_6bit_samples(bytes: &mut [u8]) -> InflatedIter<'_> {
        bytes.chunks_exact(3).flat_map(|b| {
            let s = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);

            vec![
                ((s & 0xfc0000) >> 8) as i16,
                ((s & 0x03f000) >> 2) as i16,
                ((s & 0x000fc0) << 4) as i16,
                ((s & 0x00003f) << 10) as i16,
            ]
        })
    }

    fn inflate_8bit_samples(bytes: &mut [u8]) -> InflatedIter<'_> {
        bytes
            .chunks_exact(1)
            .flat_map(|b| vec![((b[0] as u16) << 8) as i16])
    }

    pub fn decode(self) -> (Format, impl Iterator<Item = Result<DecodedSample, Error>>) {
        // [x] Try either iter returning vec
        // [ ] Or iter from_fn holding vec
        // [ ] Or passing in callback
        let inflate_fn = match self.bits {
            BitDepth::Four => Self::inflate_4bit_samples,
            BitDepth::Six => Self::inflate_6bit_samples,
            BitDepth::Eight => Self::inflate_8bit_samples,
        };
        let mut block = vec![0; self.block_len];
        let Self {
            mut reader,
            mut channels_state,
            ..
        } = self;

        let samples_per_channel = self.fmt.samples_per_channel.get();
        let channels = self.fmt.channels.get() as usize;

        let full_blocks_per_channel = samples_per_channel / SAMPLES_PER_BLOCK;
        let remainder_samples = samples_per_channel % SAMPLES_PER_BLOCK;

        let mut inflated_iter = iter::repeat_n(SAMPLES_PER_BLOCK, full_blocks_per_channel)
            .chain(iter::once(remainder_samples))
            .flat_map(move |samples| (0..channels).map(move |channel| (channel, samples)))
            .map(move |(channel, samples)| {
                reader.read_exact(&mut block)?;
                let (&mut profile, sample_bytes) = block.split_first_mut().unwrap();

                let factor = profile as usize >> 4;
                let range = profile & 0x0f;

                Result::<_, Error>::Ok(
                    inflate_fn(sample_bytes)
                        .map(move |inflated| (channel, factor, range, inflated))
                        .take(samples)
                        .collect::<Vec<_>>(),
                )
            });

        let flattened_inflated_iter = iter::from_fn({
            let mut inner_iter = None;
            move || loop {
                if let Some(item) = inner_iter.as_mut().and_then(Iterator::next) {
                    return Some(Ok(item));
                }

                match inflated_iter.next() {
                    Some(Ok(inner)) => {
                        inner_iter = Some(inner.into_iter());
                    }
                    Some(Err(e)) => return Some(Err(e)),
                    None => return None,
                }
            }
        });

        let decoded_iter = flattened_inflated_iter.map(move |inflated_res| {
            let (channel, factor, range, inflated) = inflated_res?;

            let PrevSamples(prev0, prev1) = &mut channels_state[channel];
            let (k0, k1) = GAIN_FACTOR[factor];

            let ranged = inflated as i32 >> range;
            let gain = (*prev0 as i32 * k0) + (*prev1 as i32 * k1);
            let sample_unclamped = ranged + gain / 256;
            let sample = sample_unclamped.clamp(i16::MIN as i32, i16::MAX as i32) as i16;

            *prev1 = *prev0;
            *prev0 = sample;

            Ok(DecodedSample { channel, sample })
        });

        (self.fmt, decoded_iter)
    }
}

pub fn decode_interleaved<R: Read>(reader: R) -> Result<(Format, Vec<i16>), Error> {
    let (fmt, iter) = Decoder::new(reader)?.decode();

    let samples_per_channel = fmt.samples_per_channel.get();
    let channels = fmt.channels.get().into();
    let mut pcm = vec![0; samples_per_channel * channels];
    let mut indices = (0..channels).collect::<Vec<_>>();

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        let i = &mut indices[channel];
        pcm[*i] = sample;
        *i += channels;
    }

    Ok((fmt, pcm))
}

pub fn decode_deinterleaved<R: Read>(reader: R) -> Result<(Format, Vec<Vec<i16>>), Error> {
    let (fmt, iter) = Decoder::new(reader)?.decode();

    let samples_per_channel = fmt.samples_per_channel.get();
    let channels = fmt.channels.get().into();
    let mut pcm = vec![Vec::with_capacity(samples_per_channel); channels];

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        pcm[channel].push(sample);
    }

    Ok((fmt, pcm))
}

pub fn decode_deinterleaved_f32<R: Read>(reader: R) -> Result<(Format, Vec<Vec<f32>>), Error> {
    let (fmt, iter) = Decoder::new(reader)?.decode();

    let samples_per_channel = fmt.samples_per_channel.get();
    let channels = fmt.channels.get().into();
    let mut pcm = vec![Vec::with_capacity(samples_per_channel); channels];

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        let f32_sample = sample as f32 / ((i16::MAX as f32) + 1.0);
        pcm[channel].push(f32_sample);
    }

    Ok((fmt, pcm))
}
