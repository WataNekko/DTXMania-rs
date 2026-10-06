pub mod input;

use std::{
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
};

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready},
};
use encoding_rs::SHIFT_JIS;
use ini::Ini;

use crate::assets::{config::input::InputTrigger, get_base_path, song::DrumNote};

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_config)
            .add_systems(Update, handle_load_config_task);
    }
}

#[derive(Resource)]
struct LoadConfigTask(Task<Config>);

fn load_config(mut commands: Commands) {
    let config_path = get_base_path().join("Config.ini");

    info!("Loading Config.ini");
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
    pub drum_input_map: HashMap<InputTrigger, HashSet<DrumNote>>,
}

async fn load_config_async(path: PathBuf) -> Config {
    let mut ini = async_fs::read(path)
        .await
        .map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                error!("{}", e);
            }
        })
        .and_then(|data| {
            let (decoded, _, _) = SHIFT_JIS.decode(&data);
            Ini::load_from_str(decoded.as_ref()).map_err(|e| error!("{}", e))
        })
        .unwrap_or_default();

    let mut drums_key_assign = ini.delete(Some("DrumsKeyAssign")).unwrap_or_default();
    let mut drum_input_map: HashMap<InputTrigger, HashSet<DrumNote>> = HashMap::new();

    for (key, note) in [
        ("HH", DrumNote::HiHatClose),
        ("SD", DrumNote::Snare),
        ("BD", DrumNote::BassDrum),
        ("HT", DrumNote::HighTom),
        ("LT", DrumNote::LowTom),
        ("FT", DrumNote::FloorTom),
        ("CY", DrumNote::Cymbal),
        ("HO", DrumNote::HiHatOpen),
        ("RD", DrumNote::RideCymbal),
        ("LC", DrumNote::LeftCymbal),
        ("LP", DrumNote::LeftPedal),
        ("LBD", DrumNote::LeftBass),
    ] {
        if let Some(value) = drums_key_assign.remove(key) {
            for (val, parse_res) in value
                .split(',')
                .map(str::trim)
                .map(|val| (val, InputTrigger::try_from(val)))
            {
                match parse_res {
                    Ok(trigger) => {
                        drum_input_map.entry(trigger).or_default().insert(note);
                    }
                    Err(e) => warn!("Malformed InputTrigger in Config.ini \"{}\": {}", val, e),
                }
            }
        }
    }

    Config { drum_input_map }
}
