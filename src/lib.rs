mod gameplay;
mod menu;
mod song;

use bevy::prelude::*;

use crate::{
    gameplay::GameplayPlugin, menu::song_select::SongSelectPlugin, song::scan::SongScanPlugin,
};

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((SongScanPlugin, SongSelectPlugin, GameplayPlugin))
            .init_state::<GameState>();
    }
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    SongSelect,
    Gameplay,
}
