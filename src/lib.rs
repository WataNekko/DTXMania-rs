mod gameplay;
mod menu;
mod song;

use bevy::prelude::*;

use crate::{
    gameplay::{GameplayPlugin, SongPlaying},
    menu::song_select::*,
    song::{SongDatabase, scan::*},
};

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
            GameplayPlugin {
                on_state: GameState::Gameplay,
                return_state: GameState::SongSelect,
            },
        ))
        .init_state::<GameState>()
        .add_observer(on_confirm_song_select);
    }
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    SongSelect,
    Gameplay,
}

fn on_confirm_song_select(
    select: On<ConfirmSongSelect>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    song_db: Res<SongDatabase>,
) {
    info!("Selected song: {}", song_db[select.db_idx].display());
    commands.insert_resource(SongPlaying {
        db_idx: select.db_idx,
    });
    next_state.set(GameState::Gameplay);
}
