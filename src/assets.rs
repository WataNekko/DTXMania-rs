mod audio;
pub mod config;
mod reader;
pub mod song;

use std::{env, path::PathBuf};

use bevy::prelude::*;
use ffmpeg_next as ffmpeg;

use crate::assets::audio::AudioPlugin;

pub use self::reader::{DTX_SOURCE_ID, DtxAssetReaderPlugin};

pub struct DtxAssetPlugin;

impl Plugin for DtxAssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AudioPlugin)
            .add_systems(Startup, init_ffmpeg);
    }
}

fn init_ffmpeg() -> Result {
    ffmpeg::init()?;
    Ok(())
}

fn get_base_path() -> PathBuf {
    #[cfg(feature = "dev")]
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        use std::path::PathBuf;

        return PathBuf::from(manifest_dir);
    }

    env::current_exe()
        .map(|path| path.parent().map(ToOwned::to_owned).unwrap())
        .unwrap()
}
