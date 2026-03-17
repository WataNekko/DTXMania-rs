use std::path::PathBuf;

use bevy::prelude::*;

/// All songs metadata loaded into memory.
#[derive(Resource, Deref, Reflect)]
#[reflect(Resource)]
pub struct SongDatabase(pub Vec<PathBuf>);

/// The selected song to play from the [SongDatabase].
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct SongPlaying {
    pub db_idx: usize,
}
