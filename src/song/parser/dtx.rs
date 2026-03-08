mod commands;

use std::{cmp::Ordering, collections::HashMap, io};

use bevy::{prelude::warn, tasks::futures_lite::AsyncBufRead};
use encoding_rs::SHIFT_JIS;
use nom::{
    Err, Parser,
    character::complete::space0,
    combinator::{all_consuming, eof, opt},
    error::Error,
    sequence::terminated,
};

use crate::utils::{encoding::AsyncBufReadEncodingExt, parser::*};

use self::commands::*;

pub async fn parse_dtx_chart(reader: impl AsyncBufRead + Unpin) -> io::Result<DtxChart> {
    let mut reader = reader.with_encoding(SHIFT_JIS);

    let mut parser = DtxChartParser::default();
    let mut line = String::new();

    loop {
        line.clear();

        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            break;
        }

        if let Err(err) = parser.parse_line(line.trim_end()) {
            warn!("{:?}. Ignoring line: '{}'", err, line.trim_end());
        }
    }

    Ok(parser.compile())
}

#[derive(Debug)]
pub struct DtxChart {
    pub title: String,
    pub chips: Vec<Chip>,
}

#[derive(Debug)]
pub struct Chip {
    pub time_ms: f64,
    pub channel: Channel,
    pub value: u16,
}

#[derive(Debug)]
struct DtxChartParser {
    title: String,
    curr_bpm: f64,
    bpms: HashMap<u16, f64>,
    base_bpm: f64,
    objects: Vec<Object>,
}

const DEFAULT_BPM: f64 = 120.0;

impl Default for DtxChartParser {
    fn default() -> Self {
        Self {
            title: String::new(),
            curr_bpm: DEFAULT_BPM,
            bpms: HashMap::new(),
            base_bpm: 0.0,
            objects: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct Object {
    measure: u16,
    /// In case [Object::channel] is [Channel::BarLength], this field is used for the new bar length value.
    fraction: f64,
    channel: Channel,
    value: u16,
}

impl DtxChartParser {
    fn parse_line<'a>(&mut self, input: &'a str) -> Result<(), ParseError<'a>> {
        let (input, _) = space0(input)?;

        if let (_, Some(_)) = opt(all_consuming(comment.or(eof))).parse(input)? {
            return Ok(());
        };

        let (_, (command, value)) = all_consuming(terminated(command, opt(comment)))
            .parse(input)
            .map_err(|_| ParseError::NotCommand)?;

        if self.try_parse_headers(command, value)? || self.try_parse_object_desc(command, value)? {
            Ok(())
        } else {
            Err(ParseError::UnknownCommand(command))
        }
    }

    fn try_parse_headers<'a>(
        &mut self,
        command: &'a str,
        value: &'a str,
    ) -> Result<bool, ParseError<'a>> {
        if let Some(title) = opt_err(title(command, value))? {
            self.title = title.to_string();
        } else if let Some((zz, bpm)) = opt_err(bpm(command, value))? {
            if zz == 0 {
                self.curr_bpm = bpm;
            } else {
                self.bpms.insert(zz, bpm);
            }
        } else if let Some(base_bpm) = opt_err(base_bpm(command, value))? {
            self.base_bpm = base_bpm;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn try_parse_object_desc<'a>(
        &mut self,
        command: &'a str,
        value: &'a str,
    ) -> Result<bool, ParseError<'a>> {
        let Some(ObjectDesc {
            measure,
            channel,
            value,
        }) = opt_err(object_desc(command, value))?
        else {
            return Ok(false);
        };

        if channel == Channel::BarLength {
            let value = value.into_float()?;

            self.objects.push(Object {
                measure,
                channel: Channel::BarLength,
                fraction: value, // use fraction as the bar length value
                value: 0,
            });
        } else {
            // The strategy is to push new objects to the list as we parse through the iterator.
            // But if it fails later, we'll roll back the list.

            let old_len = self.objects.len();
            let mut total_items = 0;

            let mut iter = value.into_iter();

            for (i, obj) in iter.by_ref().enumerate() {
                total_items += 1;

                if obj == 0 {
                    // Only store non-spacing objects
                    continue;
                }

                self.objects.push(Object {
                    measure,
                    fraction: i as f64,
                    channel,
                    value: obj,
                });
            }

            iter.finish().inspect_err(|_| {
                // Parsing failed. Roll back
                self.objects.truncate(old_len);
            })?;

            // All good. Post-process the new objects
            for new_obj in &mut self.objects[old_len..] {
                new_obj.fraction /= total_items as f64;
            }
        }

        Ok(true)
    }

    fn compile(self) -> DtxChart {
        let Self {
            title,
            mut curr_bpm,
            bpms,
            base_bpm,
            mut objects,
        } = self;
        let mut curr_bar_len = 1.0;

        objects.sort();

        let calc_measure_time_ms = |bar_len, bpm| {
            let beats_in_measure = bar_len * 4.0;
            let beat_time_ms = 60_000.0 / bpm;
            beats_in_measure * beat_time_ms
        };

        // These anchors are only updated when bar length or BPM change. Time of the chips are
        // calculated according to these anchors instead of the previous chip's time to reduce
        // time drift caused by accumulated float error.
        let mut anchor_measure = 0.0;
        let mut anchor_measure_time_ms = calc_measure_time_ms(curr_bar_len, curr_bpm);
        let mut anchor_time_ms = 0.0;

        let mut chips = Vec::new();

        for Object {
            measure,
            fraction,
            channel,
            value,
        } in objects
        {
            let measure =
                (measure as f64) + fraction * ((channel != Channel::BarLength) as u8 as f64);

            let measure_diff = measure - anchor_measure;

            let time_diff_ms = measure_diff * anchor_measure_time_ms;
            let time_ms = anchor_time_ms + time_diff_ms;

            let anchor_should_change = match channel {
                Channel::BarLength => {
                    let bar_len = fraction;
                    std::mem::replace(&mut curr_bar_len, bar_len) != bar_len
                }
                Channel::Bpm => {
                    let bpm = base_bpm + value as f64;
                    std::mem::replace(&mut curr_bpm, bpm) != bpm
                }
                Channel::BpmExt => {
                    let bpm = base_bpm + bpms.get(&value).unwrap_or(&DEFAULT_BPM);
                    std::mem::replace(&mut curr_bpm, bpm) != bpm
                }
                _ => {
                    chips.push(Chip {
                        time_ms,
                        channel,
                        value,
                    });

                    false
                }
            };

            if anchor_should_change {
                anchor_measure = measure;
                anchor_measure_time_ms = calc_measure_time_ms(curr_bar_len, curr_bpm);
                anchor_time_ms = time_ms;
            }
        }

        DtxChart { title, chips }
    }
}

#[derive(Debug)]
#[allow(dead_code)] // We use the invariants for logging
enum ParseError<'a> {
    NotCommand,
    UnknownCommand(&'a str),
    InvalidCommandValue(Err<Error<&'a str>>),
}

impl<'a> From<Err<Error<&'a str>>> for ParseError<'a> {
    fn from(value: Err<Error<&'a str>>) -> Self {
        Self::InvalidCommandValue(value)
    }
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
