use std::path::PathBuf;

use bevy::prelude::*;

/// All songs metadata loaded into memory.
#[derive(Resource, Deref)]
pub struct SongDatabase(pub Vec<PathBuf>);

/// The selected song to play from the [SongDatabase].
#[derive(Resource)]
pub struct SongPlaying {
    pub db_idx: usize,
}
