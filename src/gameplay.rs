mod loading;
mod playing;

use bevy::prelude::*;

use crate::{
    GameState,
    gameplay::{loading::LoadingPlugin, playing::PlayingPlugin},
    song::DtxChart,
};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((LoadingPlugin, PlayingPlugin))
            .init_state::<GameplayState>()
            .add_systems(OnEnter(GameState::Gameplay), setup)
            .add_observer(on_return);

        #[cfg(feature = "dev")]
        {
            use crate::debug::toggle_inspector;
            use bevy_inspector_egui::quick::{ResourceInspectorPlugin, StateInspectorPlugin};

            app.add_plugins((
                StateInspectorPlugin::<GameplayState>::default()
                    .run_if(toggle_inspector().and(in_state(GameState::Gameplay))),
                ResourceInspectorPlugin::<LoadedSong>::default()
                    .run_if(toggle_inspector().and(resource_exists::<LoadedSong>)),
            ));
        }
    }
}

#[derive(States, Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
enum GameplayState {
    Loading,
    Playing,
    #[default]
    Disabled,
}

#[derive(Resource, Reflect)]
struct LoadedSong {
    id: usize,
    chart: DtxChart,
}

#[derive(Event)]
struct Return;

fn setup(mut state: ResMut<NextState<GameplayState>>) {
    state.set(GameplayState::Loading);
}

fn on_return(
    _: On<Return>,
    mut game_state: ResMut<NextState<GameState>>,
    mut play_state: ResMut<NextState<GameplayState>>,
) {
    game_state.set(GameState::SongSelect);
    play_state.set(GameplayState::Disabled);
}
