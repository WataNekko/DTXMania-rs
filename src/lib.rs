mod menu;
mod song;

use bevy::prelude::*;

use crate::{menu::song_select::SongSelectPlugin, song::scan::*};

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SongScanPlugin {
                state: SongScanState::Scanning,
            },
            SongSelectPlugin {
                on_state: GameState::SongSelect,
            },
        ))
        .init_state::<GameState>();
    }
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    SongSelect,
    Gameplay,
}
