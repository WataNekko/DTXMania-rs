mod assets;
#[cfg(feature = "dev")]
mod debug;
mod gameplay;
mod menu;

use bevy::prelude::*;

use crate::{
    assets::song::SongScanPlugin, gameplay::GameplayPlugin, menu::song_select::SongSelectPlugin,
};

pub use crate::assets::{DtxAssetPlugin, DtxAssetReaderPlugin};

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "dev")]
        app.add_plugins(debug::plugin);

        app.add_plugins((SongScanPlugin, SongSelectPlugin, GameplayPlugin))
            .init_state::<GameState>();
    }
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Default, Reflect)]
enum GameState {
    #[default]
    SongSelect,
    Gameplay,
}
