use std::{
    cmp::Ordering,
    num::{NonZero, NonZeroU16},
};

use bevy::reflect::Reflect;
use nom::{
    Err, IResult, Parser,
    bytes::complete::{is_a, is_not, tag, tag_no_case, take},
    character::complete::{anychar, not_line_ending, space0},
    combinator::{all_consuming, cut, opt, recognize},
    error::{Error, ErrorKind},
    multi::fold_many0,
    sequence::{preceded, separated_pair, terminated},
};

pub fn comment(input: &str) -> IResult<&str, &str> {
    recognize((tag(";"), not_line_ending)).parse(input)
}

pub fn command(input: &str) -> IResult<&str, (&str, &str)> {
    preceded(
        tag("#"),
        cut(separated_pair(
            is_not(": \t;\r\n"),
            opt(tag(":")).and(space0),
            opt(is_not(";\r\n"))
                .map(Option::unwrap_or_default)
                .map(str::trim_end),
        )),
    )
    .parse(input)
}

type CommandResult<'a, O> = Result<O, Err<Error<&'a str>>>;

fn parse_command_tag<'a>(command: &'a str, tag: &'static str) -> CommandResult<'a, ()> {
    if command.eq_ignore_ascii_case(tag) {
        Ok(())
    } else {
        Err(Err::Error(Error::new(command, ErrorKind::Tag)))
    }
}

fn parse_command_tag_zz<'a>(command: &'a str, tag: &'static str) -> CommandResult<'a, NonZeroU16> {
    let (_, (zz_in, zz)) = all_consuming(preceded(
        tag_no_case(tag),
        take(2usize).map_res(|zz_in| u16::from_str_radix(zz_in, 36).map(|zz| (zz_in, zz))),
    ))
    .parse(command)?;

    NonZeroU16::new(zz).ok_or_else(|| Err::Failure(Error::new(zz_in, ErrorKind::MapOpt)))
}

fn parse_value_f64<'a>(value: &'a str) -> CommandResult<'a, f64> {
    value
        .parse()
        .map_err(|_| Err::Failure(Error::new(value, ErrorKind::Float)))
}

pub fn title<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, &'a str> {
    parse_command_tag(command, "TITLE")?;

    Ok(value)
}

pub fn bpm<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, (Option<NonZeroU16>, f64)> {
    let (_, opt_zz) = all_consuming(preceded(
        tag_no_case("BPM"),
        opt(take(2usize)).map_res(|opt| {
            opt.map(|zz_in| u16::from_str_radix(zz_in, 36).map(|zz| (zz_in, zz)))
                .transpose()
        }),
    ))
    .parse(command)?;

    let zz = opt_zz
        .map(|(zz_in, zz)| {
            NonZeroU16::new(zz).ok_or_else(|| Err::Failure(Error::new(zz_in, ErrorKind::MapOpt)))
        })
        .transpose()?;

    let value = parse_value_f64(value)?;

    Ok((zz, value))
}

pub fn base_bpm<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, f64> {
    parse_command_tag(command, "BASEBPM")?;

    parse_value_f64(value)
}

pub fn wav<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, (NonZeroU16, &'a str)> {
    let zz = parse_command_tag_zz(command, "WAV")?;

    Ok((zz, value))
}

/// Determines the type of playback event an [Object] represents in a chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    BarLength,
    Timed(TimedChannel),
}

/// [Channel]s describing events timed anywhere within a measure.
/// Basically all except for [Channel::BarLength].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedChannel {
    Bpm,
    BpmExt,
    SoundChip(SoundChipChannel),
}

impl TimedChannel {
    /// The radix of the object value associated with this channel.
    pub fn value_radix(&self) -> u32 {
        match self {
            Self::Bpm => 16,
            _ => 36,
        }
    }
}

impl From<TimedChannel> for Channel {
    fn from(value: TimedChannel) -> Self {
        Self::Timed(value)
    }
}

/// [Channel]s with audio data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundChipChannel {
    Bgm,
    Drum(DrumNote),
}

impl From<SoundChipChannel> for Channel {
    fn from(value: SoundChipChannel) -> Self {
        TimedChannel::SoundChip(value).into()
    }
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

impl From<DrumNote> for Channel {
    fn from(value: DrumNote) -> Self {
        SoundChipChannel::Drum(value).into()
    }
}

#[derive(Debug)]
pub struct UnsupportedChannelError;

impl TryFrom<u8> for Channel {
    type Error = UnsupportedChannelError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let ch = match value {
            0x01 => SoundChipChannel::Bgm.into(),
            0x02 => Channel::BarLength,
            0x03 => TimedChannel::Bpm.into(),
            0x08 => TimedChannel::BpmExt.into(),
            0x11 => DrumNote::HiHatClose.into(),
            0x12 => DrumNote::Snare.into(),
            0x13 => DrumNote::BassDrum.into(),
            0x14 => DrumNote::HighTom.into(),
            0x15 => DrumNote::LowTom.into(),
            0x16 => DrumNote::Cymbal.into(),
            0x17 => DrumNote::FloorTom.into(),
            0x18 => DrumNote::HiHatOpen.into(),
            0x19 => DrumNote::RideCymbal.into(),
            0x1A => DrumNote::LeftCymbal.into(),
            0x1B => DrumNote::LeftPedal.into(),
            0x1C => DrumNote::LeftBass.into(),
            _ => return Err(UnsupportedChannelError),
        };
        Ok(ch)
    }
}

/// Raw data describing unprocessed playback information of a chart, such as in which measure, which
/// lane, when and what happens (e.g., BGM chip gets played, BPM changes, drum chip to be hit,
/// etc.). The chart parser would process these [Object]s into chips for easy logic running during
/// playback.
///
/// Chips are playback events, making up a playable chart and describing processed information on
/// what would happen during the chart, at some given point on the playback timeline
/// (e.g., a sound chip associates the event with a sound, hittable or not).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Object {
    pub measure: u16,
    pub value: ObjectValue,
}

#[derive(Debug)]
pub enum ObjectValue {
    BarLength(f64),
    Timed {
        /// The fraction [0..1] within a measure, at which the object is timed.
        fraction: f64,
        channel: TimedChannel,
        value: NonZeroU16,
    },
}

impl PartialEq for ObjectValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BarLength(a), Self::BarLength(b)) => a.total_cmp(b).is_eq(),
            (
                Self::Timed {
                    fraction: self_frac,
                    channel: self_ch,
                    value: self_val,
                },
                Self::Timed {
                    fraction: other_frac,
                    channel: other_ch,
                    value: other_val,
                },
            ) => {
                (self_ch, self_val) == (other_ch, other_val)
                    && self_frac.total_cmp(other_frac).is_eq()
            }
            _ => false,
        }
    }
}

impl Eq for ObjectValue {}

impl PartialOrd for ObjectValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ObjectValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::BarLength(_), Self::BarLength(_)) => Ordering::Equal,
            (Self::BarLength(_), Self::Timed { .. }) => Ordering::Less,
            (Self::Timed { .. }, Self::BarLength(_)) => Ordering::Greater,
            (
                Self::Timed {
                    fraction: self_frac,
                    ..
                },
                Self::Timed {
                    fraction: other_frac,
                    ..
                },
            ) => self_frac.total_cmp(other_frac),
        }
    }
}

fn measure(input: &str) -> IResult<&str, u16> {
    (
        take(1usize).map_res(|m| u16::from_str_radix(m, 36)),
        take(2usize).map_res(str::parse::<u16>),
    )
        .map(|(m, mm)| m * 100 + mm)
        .parse(input)
}

fn channel(input: &str) -> IResult<&str, Channel> {
    let (rem, channel) = take(2usize)
        .map_res(|cc| u8::from_str_radix(cc, 16))
        .parse(input)?;

    channel
        .try_into()
        .map(|ch| (rem, ch))
        .map_err(|_| Err::Failure(Error::new(input, ErrorKind::MapRes)))
}

pub fn object_desc<'a>(
    command: &'a str,
    value: &'a str,
    out: &mut Vec<Object>,
) -> CommandResult<'a, ()> {
    let (_, (measure, channel)) = all_consuming((measure, channel)).parse(command)?;

    match channel {
        Channel::BarLength => {
            let value = value
                .parse()
                .map_err(|_| Err::Failure(Error::new(value, ErrorKind::Float)))?;

            out.push(Object {
                measure,
                value: ObjectValue::BarLength(value),
            });
        }
        Channel::Timed(channel) => {
            // For rolling back the list later if parsing fails.
            let old_len = out.len();

            let radix = channel.value_radix();
            let digit = move |i| anychar.map_opt(|c| c.to_digit(radix)).parse(i);
            let digit_ignoring_underscore = move |i| digit.or(preceded(is_a("_"), digit)).parse(i);

            let object_list = fold_many0(
                (digit_ignoring_underscore, digit_ignoring_underscore),
                || 0.0,
                |fraction, (d1, d2)| {
                    let value = (d1 * radix + d2) as u16;

                    if let Some(value) = NonZero::new(value) {
                        // Only store non-spacing values
                        out.push(Object {
                            measure,
                            value: ObjectValue::Timed {
                                fraction,
                                channel,
                                value,
                            },
                        });
                    }

                    fraction + 1.0
                },
            );

            let result = cut(all_consuming(terminated(object_list, opt(is_a("_"))))).parse(value);

            let total_items = match result {
                Ok((_, i)) => i,
                Err(e) => {
                    // Parsing failed. Roll back
                    out.truncate(old_len);
                    return Err(e);
                }
            };

            // All good. Post-process the new objects
            for new_obj in &mut out[old_len..] {
                match &mut new_obj.value {
                    ObjectValue::Timed { fraction, .. } => {
                        *fraction /= total_items;
                    }
                    _ => unreachable!("We only pushed ObjectValue::Timed here."),
                }
            }
        }
    }

    Ok(())
}
