mod audio;
mod reader;
pub mod song;

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
