use std::cmp::Ordering;

use bevy::{asset::Handle, reflect::Reflect};
use bevy_seedling::sample::AudioSample;

/// Partially interpreted/processed info on what feature/chip the associated object data is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    BarLength,
    Bpm,
    BpmExt,
    Sound(SoundChip),
}

#[derive(Debug)]
pub struct UnsupportedChannelError;

impl TryFrom<u8> for Channel {
    type Error = UnsupportedChannelError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Sound(SoundChip::Bgm)),
            0x02 => Ok(Self::BarLength),
            0x03 => Ok(Self::Bpm),
            0x08 => Ok(Self::BpmExt),
            0x11 => Ok(Self::Sound(SoundChip::Drum(DrumNote::HiHatClose))),
            0x12 => Ok(Self::Sound(SoundChip::Drum(DrumNote::Snare))),
            0x13 => Ok(Self::Sound(SoundChip::Drum(DrumNote::BassDrum))),
            0x14 => Ok(Self::Sound(SoundChip::Drum(DrumNote::HighTom))),
            0x15 => Ok(Self::Sound(SoundChip::Drum(DrumNote::LowTom))),
            0x16 => Ok(Self::Sound(SoundChip::Drum(DrumNote::Cymbal))),
            0x17 => Ok(Self::Sound(SoundChip::Drum(DrumNote::FloorTom))),
            0x18 => Ok(Self::Sound(SoundChip::Drum(DrumNote::HiHatOpen))),
            0x19 => Ok(Self::Sound(SoundChip::Drum(DrumNote::RideCymbal))),
            0x1A => Ok(Self::Sound(SoundChip::Drum(DrumNote::LeftCymbal))),
            0x1B => Ok(Self::Sound(SoundChip::Drum(DrumNote::LeftPedal))),
            0x1C => Ok(Self::Sound(SoundChip::Drum(DrumNote::LeftBass))),
            _ => Err(UnsupportedChannelError),
        }
    }
}

impl Channel {
    /// The radix of the object value associated with this channel.
    pub fn value_radix(&self) -> u32 {
        match self {
            Self::BarLength => 0, // value not integer
            Self::Bpm => 16,
            _ => 36,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum SoundChip {
    Bgm,
    Drum(DrumNote),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum DrumNote {
    HiHatClose,
    Snare,
    BassDrum,
    HighTom,
    LowTom,
    Cymbal,
    FloorTom,
    HiHatOpen,
    RideCymbal,
    LeftCymbal,
    LeftPedal,
    LeftBass,
}

/// Uninterpreted, raw data associated with a channel (for a feature/chip).
#[derive(Debug)]
pub struct Object {
    pub measure: u16,
    /// In case [Object::channel] is [Channel::BarLength], this field is used for the new bar length value.
    pub fraction: f64,
    pub channel: Channel,
    pub value: u16,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        (self.measure, self.channel, self.value) == (other.measure, other.channel, other.value)
            && self.fraction.total_cmp(&other.fraction).is_eq()
    }
}

impl Eq for Object {}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Object {
    fn cmp(&self, other: &Self) -> Ordering {
        self.measure.cmp(&other.measure).then_with(|| {
            match (
                self.channel == Channel::BarLength,
                other.channel == Channel::BarLength,
            ) {
                (true, true) => Ordering::Equal,
                (self_is_bar_len, other_is_bar_len) => (!self_is_bar_len)
                    .cmp(&(!other_is_bar_len))
                    .then_with(|| self.fraction.total_cmp(&other.fraction)),
            }
        })
    }
}

#[derive(Debug, Reflect)]
pub struct ChipInfo {
    pub time_sec: f64,
    pub chip: Chip,
}

/// Interpreted/processed info on what would happen at a given time during a chart. (E.g., a sound
/// chip would associate the event with a sound.)
#[derive(Debug, Reflect)]
pub enum Chip {
    Sound {
        chip: SoundChip,
        audio: Handle<AudioSample>,
    },
}
