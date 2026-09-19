use std::{
    io::{self, Read},
    num::{NonZero, NonZeroU16, NonZeroU32},
};

const HEADER_LEN: usize = 32;
const HEADER_MAGIC: u32 = u32::from_le_bytes(*b"KWD1");
const SAMPLES_PER_BLOCK: u32 = 32;
const GAIN_FACTOR: [(i32, i32); 5] = [(0, 0), (240, 0), (460, -208), (392, -220), (488, -240)];

pub enum Error {
    Io(io::Error),
    InvalidHeader,
}

pub struct Format {
    pub sample_rate: NonZeroU16,
}

struct PrevSamples(i16, i16);

pub struct Decoder<R: Read> {
    pub reader: R,
    data_len: NonZeroU32,
    block_len: u32,
    samples_per_channel: NonZeroU32,
    pub fmt: Format,
    channels_state: [PrevSamples; 2],
}

impl<R: Read> Decoder<R> {
    pub fn new(mut reader: R) -> Result<Decoder<R>, Error> {
        let mut header = [0u8; HEADER_LEN];
        reader.read_exact(&mut header).map_err(Error::Io)?;

        // Extract fields
        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let data_len = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let samples_per_channel = u32::from_le_bytes(header[8..12].try_into().unwrap());
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

        let block_len = (bits as u32) * SAMPLES_PER_BLOCK / 8 + 1;
        // total samples length per block + profile byte

        match (
            magic,
            NonZero::new(data_len),
            NonZero::new(samples_per_channel),
            NonZero::new(sample_rate),
            channels,
        ) {
            (HEADER_MAGIC, Some(data_len), Some(samples_per_channel), Some(sample_rate), 1 | 2)
                if data_len.get() % block_len == 0 && {
                    let blocks = data_len.get() / block_len;
                    let blocks_per_channel = blocks / (channels as u32);

                    let upper = blocks_per_channel * SAMPLES_PER_BLOCK;
                    let lower = upper - SAMPLES_PER_BLOCK + 1;
                    (lower..=upper).contains(&samples_per_channel.get())
                } =>
            {
                Ok(Self {
                    reader,
                    data_len,
                    block_len,
                    samples_per_channel,
                    fmt: Format { sample_rate },
                    channels_state,
                })
            }
            _ => Err(Error::InvalidHeader),
        }
    }
}
