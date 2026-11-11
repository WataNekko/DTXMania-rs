mod parsers;

use std::{collections::HashMap, io, num::NonZeroU16, path::Path};

use async_fs::File;
use bevy::{asset::AssetPath, prelude::*, reflect::Reflect, tasks::futures_lite::io::BufReader};
use bevy_seedling::{firewheel::clock::DurationSeconds, sample::AudioSample};
use encoding_rs::SHIFT_JIS;
use enum_map::EnumMap;
use nom::{
    Err, Parser,
    character::complete::space0,
    combinator::{all_consuming, eof, opt},
    error::Error,
    sequence::terminated,
};
use utils::{encoding::AsyncBufReadEncodingExt, parser::opt_err};

use crate::assets::DTX_SOURCE_ID;

use self::parsers::*;

pub use self::parsers::DrumNote;

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

    let base_path = path.parent().expect("path is a file, should have parent.");

    let (chart, asset_handles) = parser.finalize(base_path, asset_server);

    // Wait for all the assets to finish loading
    for handle in asset_handles {
        let _ = asset_server.wait_for_asset_untyped(&handle).await;
        // discard possible error since AssetServer already logs this
    }

    Ok(chart)
}

#[derive(Debug, Clone, Reflect)]
pub struct SoundChip {
    pub time: DurationSeconds,
    pub audio: Handle<AudioSample>,
}

#[derive(Debug, Clone, Reflect)]
#[reflect(opaque)]
pub struct DtxChart {
    pub title: String,
    pub bgm_chips: Vec<SoundChip>,
    pub drum_chips: EnumMap<DrumNote, Vec<SoundChip>>,
}

#[derive(Debug)]
struct DtxChartParser {
    title: String,
    bpm: f64,
    bpm_list: HashMap<NonZeroU16, f64>,
    base_bpm: f64,
    audio_list: HashMap<NonZeroU16, String>,
    objects: Vec<Object>,
}

impl Default for DtxChartParser {
    fn default() -> Self {
        Self {
            title: String::new(),
            bpm: Self::DEFAULT_BPM,
            bpm_list: HashMap::new(),
            base_bpm: 0.0,
            audio_list: HashMap::new(),
            objects: Vec::new(),
        }
    }
}

impl DtxChartParser {
    const DEFAULT_BPM: f64 = 120.0;

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
            if let Some(zz) = zz {
                self.bpm_list.insert(zz, bpm);
            } else {
                self.bpm = bpm;
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
        let parsed = opt_err(object_desc(command, value, &mut self.objects))?;
        Ok(parsed.is_some())
    }

    fn finalize(
        self,
        base_path: &Path,
        asset_server: &AssetServer,
    ) -> (DtxChart, Vec<UntypedHandle>) {
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
        // calculated according to these anchors instead of the previous chip's time to reduce time
        // drifts caused by accumulated float errors.
        let mut anchor_measure = 0.0;
        let mut curr_measure_time = calc_measure_time(curr_bar_len, curr_bpm);
        let mut anchor_time = 0.0;

        let mut audio_handles = HashMap::new();
        let mut bgm_chips = Vec::new();
        let mut drum_chips = EnumMap::<_, Vec<_>>::default();

        for Object { measure, value } in objects {
            let measure = measure as f64
                + match value {
                    ObjectValue::BarLength(_) => 0.0,
                    ObjectValue::Timed { fraction, .. } => fraction,
                };

            let measure_diff = measure - anchor_measure;

            let time_diff = measure_diff * curr_measure_time;
            let playback_time = anchor_time + time_diff;

            let mut update_anchor = |bar_len, bpm| {
                anchor_measure = measure;
                curr_measure_time = calc_measure_time(bar_len, bpm);
                anchor_time = playback_time;
            };

            let (channel, value) = match value {
                ObjectValue::BarLength(bar_len) => {
                    if curr_bar_len != bar_len {
                        curr_bar_len = bar_len;
                        update_anchor(bar_len, curr_bpm);
                    }
                    continue;
                }
                ObjectValue::Timed { channel, value, .. } => (channel, value),
            };

            match channel {
                TimedChannel::Bpm => {
                    let bpm = base_bpm + value.get() as f64;

                    if curr_bpm != bpm {
                        curr_bpm = bpm;
                        update_anchor(curr_bar_len, bpm);
                    }
                }
                TimedChannel::BpmExt => {
                    let bpm = base_bpm + bpm_list.get(&value).unwrap_or(&Self::DEFAULT_BPM);

                    if curr_bpm != bpm {
                        curr_bpm = bpm;
                        update_anchor(curr_bar_len, bpm);
                    }
                }
                TimedChannel::SoundChip(channel) => {
                    let audio = audio_handles
                        .entry(value)
                        .or_insert_with(|| {
                            audio_list
                                .get(&value)
                                .map(|name| {
                                    let path = base_path.join(name);

                                    asset_server
                                        .load(AssetPath::from(path).with_source(DTX_SOURCE_ID))
                                })
                                .unwrap_or_default()
                        })
                        .clone();

                    let chip = SoundChip {
                        time: playback_time.into(),
                        audio,
                    };

                    match channel {
                        SoundChipChannel::Bgm => bgm_chips.push(chip),
                        SoundChipChannel::Drum(note) => drum_chips[note].push(chip),
                    }
                }
            }
        }

        let asset_handles = Vec::from_iter(audio_handles.into_values().map(Handle::untyped));

        (
            DtxChart {
                title,
                bgm_chips,
                drum_chips,
            },
            asset_handles,
        )
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
