mod menu;
mod song;

use bevy::prelude::*;

use crate::{menu::song_select::*, song::scan::*};

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
    mut next_state: ResMut<NextState<GameState>>,
) {
    info!("{:?}", *select);
    next_state.set(GameState::Gameplay);
}
