mod chips;
mod parsers;

use std::{collections::HashMap, io, path::Path};

use async_fs::File;
use bevy::{
    asset::UntypedAssetId, prelude::*, reflect::Reflect, tasks::futures_lite::io::BufReader,
};
use encoding_rs::SHIFT_JIS;
use nom::{
    Err, Parser,
    character::complete::space0,
    combinator::{all_consuming, eof, opt},
    error::Error,
    sequence::terminated,
};

use crate::utils::{encoding::AsyncBufReadEncodingExt, parser::*};

use self::{
    chips::{Channel, Object},
    parsers::*,
};

pub use self::chips::{Chip, ChipInfo, DrumNote, SoundChip};

pub async fn load_dtx_chart(
    path: impl AsRef<Path>,
    asset_server: &AssetServer,
) -> io::Result<DtxChart> {
    let path = path.as_ref();
    let file = File::open(path).await?;
    let mut reader = BufReader::new(file).with_encoding(SHIFT_JIS);

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

    let base_path = path
        .parent()
        .expect("Since path sure is a file at this point, getting the parent dir shouldn't fail.");

    let (chart, asset_ids) = parser.compile(base_path, asset_server);

    // Wait for all the assets to finish loading
    for id in asset_ids {
        let _ = asset_server.wait_for_asset_id(id).await;
    }

    Ok(chart)
}

#[derive(Debug, Reflect)]
pub struct DtxChart {
    pub title: String,
    pub chips: Vec<ChipInfo>,
}

#[derive(Debug)]
struct DtxChartParser {
    title: String,
    bpm: f64,
    bpm_list: HashMap<u16, f64>,
    base_bpm: f64,
    audio_list: HashMap<u16, String>,
    objects: Vec<Object>,
}

const DEFAULT_BPM: f64 = 120.0;

impl Default for DtxChartParser {
    fn default() -> Self {
        Self {
            title: String::new(),
            bpm: DEFAULT_BPM,
            bpm_list: HashMap::new(),
            base_bpm: 0.0,
            audio_list: HashMap::new(),
            objects: Vec::new(),
        }
    }
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
                self.bpm = bpm;
            } else {
                self.bpm_list.insert(zz, bpm);
            }
        } else if let Some(base_bpm) = opt_err(base_bpm(command, value))? {
            self.base_bpm = base_bpm;
        } else if let Some((zz, name)) = opt_err(wav(command, value))? {
            self.audio_list.insert(zz, name.to_string());
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

    fn compile(
        self,
        base_path: &Path,
        asset_server: &AssetServer,
    ) -> (DtxChart, Vec<UntypedAssetId>) {
        let Self {
            title,
            bpm: mut curr_bpm,
            bpm_list,
            base_bpm,
            audio_list,
            mut objects,
        } = self;
        let mut curr_bar_len = 1.0;

        objects.sort();

        let calc_measure_time = |bar_len, bpm| {
            let beats_in_measure = bar_len * 4.0;
            let beat_time = 60.0 / bpm;
            beats_in_measure * beat_time
        };

        // These anchors are only updated when bar length or BPM change. Time of the chips are
        // calculated according to these anchors instead of the previous chip's time to reduce
        // time drift caused by accumulated float error.
        let mut anchor_measure = 0.0;
        let mut anchor_measure_time = calc_measure_time(curr_bar_len, curr_bpm);
        let mut anchor_time = 0.0;

        let mut chips = Vec::new();

        let mut audio_handles = HashMap::new();

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

            let time_diff = measure_diff * anchor_measure_time;
            let time_sec = anchor_time + time_diff;

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
                    let bpm = base_bpm + bpm_list.get(&value).unwrap_or(&DEFAULT_BPM);
                    std::mem::replace(&mut curr_bpm, bpm) != bpm
                }
                Channel::Sound(chip) => {
                    let audio = audio_handles
                        .entry(value)
                        .or_insert_with(|| {
                            audio_list
                                .get(&value)
                                .map(|name| base_path.join(name))
                                // TODO: On case-sensitive file systems, if the case of the file
                                // name in the chart differs from the real name on disk, this would
                                // fail. Resolve this with a custom asset reader?
                                .map(|path| asset_server.load(path))
                                .unwrap_or_default()
                        })
                        .clone();

                    chips.push(ChipInfo {
                        time_sec,
                        chip: Chip::Sound { chip, audio },
                    });

                    false
                }
            };

            if anchor_should_change {
                anchor_measure = measure;
                anchor_measure_time = calc_measure_time(curr_bar_len, curr_bpm);
                anchor_time = time_sec;
            }
        }

        let asset_ids = Vec::from_iter(audio_handles.into_values().map(|h| h.id().untyped()));

        (DtxChart { title, chips }, asset_ids)
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
