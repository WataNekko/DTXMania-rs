pub mod input;

use std::{borrow::Cow, io, path::PathBuf};

use async_fs::File;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready, futures_lite::io::BufReader},
};
use encoding_rs::SHIFT_JIS;
use nom::{
    IResult, Parser,
    bytes::complete::{is_not, tag},
    character::complete::space0,
    combinator::{all_consuming, recognize, rest},
    sequence::{delimited, separated_pair},
};
use utils::encoding::AsyncBufReadEncodingExt;

use crate::assets::{
    config::input::{DrumInputMap, DrumInputMapParser},
    get_base_path,
};

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_config).add_systems(
            Update,
            (
                handle_load_config_task,
                log_config.run_if(resource_exists_and_changed::<Config>),
            ),
        );
    }
}

#[derive(Resource)]
struct LoadConfigTask(Task<Config>);

fn load_config(mut commands: Commands) {
    let config_path = get_base_path().join("Config.ini");

    info!("Loading Config.ini...");
    let task = IoTaskPool::get().spawn(load_config_async(config_path));

    commands.insert_resource(LoadConfigTask(task));
}

fn handle_load_config_task(mut commands: Commands, mut task: If<ResMut<LoadConfigTask>>) {
    if let Some(config) = check_ready(&mut task.0.0) {
        commands.remove_resource::<LoadConfigTask>();
        commands.insert_resource(config);
        info!("Config.ini loaded");
    }
}

#[derive(Resource, Debug)]
pub struct Config {
    pub drum_input_map: DrumInputMap,
}

async fn load_config_async(path: PathBuf) -> Config {
    let mut drum_input_map_parser = DrumInputMapParser::new();

    match File::open(path).await {
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                error!("Error opening Config.ini: {}", e);
            }
        }
        Ok(file) => {
            let mut reader = BufReader::new(file).with_encoding(SHIFT_JIS);
            let mut line = String::new();
            let mut line_number = 0usize;
            let mut section_parser: Option<&mut dyn SectionParser> = None;

            while reader
                .read_line(&mut line)
                .await
                .inspect_err(|e| error!("Error reading Config.ini {}", e))
                .is_ok_and(|read| read > 0)
            {
                line_number += 1;
                let input = line.trim();

                if input.is_empty() || comment_line(input).is_ok() {
                } else if let Ok((_, section)) = section_header(input) {
                    section_parser = match section {
                        "DrumsKeyAssign" => Some(&mut drum_input_map_parser),
                        _ => {
                            warn!("Ignoring unknown section: [{}]", section);
                            None
                        }
                    };
                } else if let Ok((_, (key, value))) = entry_line(input) {
                    if let Some(section) = section_parser.as_mut() {
                        let res = section.parse_entry(key, value);

                        if let Err(e) = res {
                            warn!("Line #{}: {}: {}", line_number, e, input);
                        }
                    }
                } else {
                    warn!("Invalid line #{} in Config.ini: {}", line_number, input);
                }

                line.clear();
            }
        }
    }

    Config {
        drum_input_map: drum_input_map_parser.finalize(),
    }
}

fn comment_line(input: &str) -> IResult<&str, &str> {
    recognize((tag(";"), rest)).parse(input)
}

fn section_header(input: &str) -> IResult<&str, &str> {
    all_consuming(delimited(tag("["), is_not("]"), tag("]")))
        .map(str::trim)
        .parse(input)
}

fn entry_line(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(
        Parser::map(is_not("="), str::trim_end),
        (tag("="), space0),
        rest,
    )
    .parse(input)
}

trait SectionParser: Send {
    fn parse_entry<'a>(&mut self, key: &'a str, value: &'a str) -> Result<(), Cow<'a, str>>;
}

fn log_config(config: Res<Config>) {
    use std::fmt::Write;
    let mut s = String::new();

    writeln!(&mut s, "Config:").unwrap();
    writeln!(&mut s, "[DrumsKeyAssign]").unwrap();
    for (note, _, _) in DrumInputMapParser::CONFIG_KEYS_AND_DEFAULTS {
        let list = config.drum_input_map.trigger_list(note);
        let list = list
            .iter()
            .map(|t| t.map_or_default(|t| t.in_game_display().to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(&mut s, "{:?}={}", note, list).unwrap();
    }

    info!("{}", s);
}
