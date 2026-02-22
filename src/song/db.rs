use std::path::PathBuf;

use bevy::prelude::*;

/// May not exist if song scan is not done.
#[derive(Resource, Deref)]
pub struct SongDatabase(pub Vec<PathBuf>);
