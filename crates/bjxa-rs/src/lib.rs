use std::{
    io::{self, Read},
    iter,
    num::{NonZero, NonZeroU16},
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
    pub samples_per_channel: usize,
    pub sample_rate: NonZeroU16,
    pub channels: usize,
}

type InflateSamplesFn = fn(&[u8], &mut Vec<i16>) -> Option<usize>;

struct PrevSamples(i16, i16);

pub struct DecodedSample {
    channel: usize,
    sample: i16,
}

pub struct Decoder<R: Read> {
    pub reader: R,
    fmt: Format,
    block_len: usize,
    inflate_samples_fn: InflateSamplesFn,
    channels_state: [PrevSamples; 2],
}

impl<R: Read> Decoder<R> {
    pub fn new(mut reader: R) -> Result<Decoder<R>, Error> {
        let mut header = [0u8; HEADER_LEN];
        reader.read_exact(&mut header)?;

        // Extract fields
        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let data_len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let samples_per_channel = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let sample_rate = u16::from_le_bytes(header[12..14].try_into().unwrap());
        let bit_depth = header[14];
        let channels = header[15] as usize;
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

        let block_len = (bit_depth as usize) * SAMPLES_PER_BLOCK / 8 + 1;
        // total samples length per block + profile byte

        match (
            magic,
            data_len,
            samples_per_channel,
            NonZero::new(sample_rate),
            Self::get_inflate_samples_fn(bit_depth),
            channels,
        ) {
            (HEADER_MAGIC, 1.., 1.., Some(sample_rate), Some(inflate_samples_fn), 1 | 2)
                if data_len.is_multiple_of(block_len) && {
                    let blocks = data_len / block_len;
                    let blocks_per_channel = blocks / channels;

                    let upper = blocks_per_channel * SAMPLES_PER_BLOCK;
                    let lower = upper - SAMPLES_PER_BLOCK + 1;
                    (lower..=upper).contains(&samples_per_channel)
                } =>
            {
                Ok(Self {
                    reader,
                    fmt: Format {
                        samples_per_channel,
                        sample_rate,
                        channels,
                    },
                    block_len,
                    inflate_samples_fn,
                    channels_state,
                })
            }
            _ => Err(Error::InvalidHeader),
        }
    }

    pub fn format(&mut self) -> Format {
        self.fmt
    }

    fn get_inflate_samples_fn(bit_depth: u8) -> Option<InflateSamplesFn> {
        match bit_depth {
            4 => Some(Self::inflate_4bit_samples),
            6 => Some(Self::inflate_6bit_samples),
            8 => Some(Self::inflate_8bit_samples),
            _ => None,
        }
    }

    fn inflate_4bit_samples(bytes: &[u8], out_stack: &mut Vec<i16>) -> Option<usize> {
        let consumed @ &[b] = bytes.first_chunk()?;
        let b = b as u16;

        out_stack.push(((b & 0x0f) << 12) as i16);
        out_stack.push(((b & 0xf0) << 8) as i16);

        Some(consumed.len())
    }

    fn inflate_6bit_samples(bytes: &[u8], out_stack: &mut Vec<i16>) -> Option<usize> {
        let consumed @ &[b0, b1, b2] = bytes.first_chunk()?;
        let b = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        out_stack.push(((b & 0x00003f) << 10) as i16);
        out_stack.push(((b & 0x000fc0) << 4) as i16);
        out_stack.push(((b & 0x03f000) >> 2) as i16);
        out_stack.push(((b & 0xfc0000) >> 8) as i16);

        Some(consumed.len())
    }

    fn inflate_8bit_samples(bytes: &[u8], out_stack: &mut Vec<i16>) -> Option<usize> {
        let consumed @ &[b] = bytes.first_chunk()?;
        let b = b as u16;

        out_stack.push((b << 8) as i16);

        Some(consumed.len())
    }

    pub fn decode(self) -> (Format, impl Iterator<Item = Result<DecodedSample, Error>>) {
        let Self {
            mut reader,
            fmt:
                Format {
                    samples_per_channel,
                    channels,
                    ..
                },
            block_len,
            inflate_samples_fn,
            mut channels_state,
        } = self;

        let full_blocks_per_channel = samples_per_channel / SAMPLES_PER_BLOCK;
        let remainder_samples = samples_per_channel % SAMPLES_PER_BLOCK;
        let opt_remainder_samples = NonZero::new(remainder_samples).map(NonZero::get);

        let mut block_iter = iter::repeat_n(SAMPLES_PER_BLOCK, full_blocks_per_channel)
            .chain(opt_remainder_samples)
            .flat_map(move |samples| (0..channels).map(move |channel| (channel, samples)));

        let inflated_iter = iter::from_fn({
            let mut block = vec![0; block_len];
            let mut block_offset = 0;
            let mut inflated_stack = Vec::new();

            let mut curr_channel = 0;
            let mut factor = 0;
            let mut range = 0;
            let mut remaining_samples = 0;

            move || loop {
                if remaining_samples > 0 {
                    if let Some(inflated) = inflated_stack.pop() {
                        remaining_samples -= 1;
                        return Some(Ok((curr_channel, factor, range, inflated)));
                    }

                    if let Some(consumed) =
                        inflate_samples_fn(&block[block_offset..], &mut inflated_stack)
                    {
                        block_offset += consumed;
                        continue;
                    }
                }

                match block_iter.next() {
                    Some((channel, samples)) => {
                        if let Err(e) = reader.read_exact(&mut block) {
                            return Some(Err(e));
                        }

                        let profile = block[0];
                        factor = profile as usize >> 4;
                        range = profile & 0x0f;

                        block_offset = 1;
                        inflated_stack.clear();

                        curr_channel = channel;
                        remaining_samples = samples;
                    }
                    None => return None,
                }
            }
        });

        let decoded_iter = inflated_iter.map(move |inflated_res| {
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

    let mut pcm = vec![0; fmt.samples_per_channel * fmt.channels];
    let mut indices = (0..fmt.channels).collect::<Vec<_>>();

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        let i = &mut indices[channel];
        pcm[*i] = sample;
        *i += fmt.channels;
    }

    Ok((fmt, pcm))
}

pub fn decode_deinterleaved<R: Read>(reader: R) -> Result<(Format, Vec<Vec<i16>>), Error> {
    let (fmt, iter) = Decoder::new(reader)?.decode();

    let mut pcm = vec![Vec::with_capacity(fmt.samples_per_channel); fmt.channels];

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        pcm[channel].push(sample);
    }

    Ok((fmt, pcm))
}

pub fn decode_deinterleaved_f32<R: Read>(reader: R) -> Result<(Format, Vec<Vec<f32>>), Error> {
    let (fmt, iter) = Decoder::new(reader)?.decode();

    let mut pcm = vec![Vec::with_capacity(fmt.samples_per_channel); fmt.channels];

    for res in iter {
        let DecodedSample { channel, sample } = res?;
        let f32_sample = sample as f32 / ((i16::MAX as f32) + 1.0);
        pcm[channel].push(f32_sample);
    }

    Ok((fmt, pcm))
}
