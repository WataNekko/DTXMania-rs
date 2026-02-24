use std::{io, time::Duration};

use bevy::{
    prelude::*,
    state::state::FreelyMutableState,
    tasks::{IoTaskPool, Task, futures::check_ready},
    time::common_conditions::on_timer,
};

use crate::song::SongDatabase;

pub struct GameplayPlugin<S> {
    pub on_state: S,
    pub return_state: S,
}

impl<S: States + FreelyMutableState + Copy> Plugin for GameplayPlugin<S> {
    fn build(&self, app: &mut App) {
        app.init_state::<GameplayState>()
            .add_systems(OnEnter(self.on_state), setup)
            .add_systems(OnExit(self.on_state), cleanup)
            .add_systems(OnEnter(GameplayState::Loading), load_song)
            .add_systems(Update, handle_song_load_done)
            .add_systems(OnEnter(GameplayState::Play), play_setup)
            .add_systems(
                Update,
                (|mut commands: Commands| commands.trigger(StateReturn))
                    .run_if(in_state(GameplayState::Play).and(on_timer(Duration::from_secs(5)))),
            )
            .add_observer(on_state_return(self.return_state));
    }
}

#[derive(States, Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
enum GameplayState {
    Loading,
    Play,
    #[default]
    Disabled,
}

/// This must be inserted before transitioning into the Gameplay state.
#[derive(Resource)]
pub struct SongPlaying {
    pub db_idx: usize,
}

#[derive(Resource)]
struct SongLoadTask(Task<io::Result<String>>);

#[derive(Resource)]
struct LoadedSong(String);

#[derive(Event)]
struct StateReturn;

fn setup(mut state: ResMut<NextState<GameplayState>>) {
    state.set(GameplayState::Loading);
}

fn cleanup(mut state: ResMut<NextState<GameplayState>>) {
    state.set(GameplayState::default());
}

fn load_song(mut commands: Commands, song_playing: Res<SongPlaying>, song_db: Res<SongDatabase>) {
    let song = song_db[song_playing.db_idx].clone();
    info!("Loading song: {}", song.display());

    let task = IoTaskPool::get().spawn(async move { async_fs::read_to_string(song).await });

    commands.insert_resource(SongLoadTask(task));
}

fn handle_song_load_done(
    mut commands: Commands,
    mut song_load: If<ResMut<SongLoadTask>>,
    mut next_state: ResMut<NextState<GameplayState>>,
) {
    let Some(res) = check_ready(&mut song_load.0.0) else {
        return;
    };
    commands.remove_resource::<SongLoadTask>();

    match res {
        Err(err) => {
            error!("Error loading song: {}", err);
            commands.trigger(StateReturn);
        }
        Ok(text) => {
            commands.insert_resource(LoadedSong(text));
            next_state.set(GameplayState::Play);
        }
    }
}

fn on_state_return<S: States + FreelyMutableState + Copy>(
    return_state: S,
) -> impl Fn(On<StateReturn>, ResMut<NextState<S>>) {
    move |_, mut next_state| {
        next_state.set(return_state);
    }
}

fn play_setup(mut commands: Commands, song: Res<LoadedSong>) {
    let text = song.0.clone();

    commands.spawn((
        DespawnOnExit(GameplayState::Play),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::right(Val::Px(50.)),
            ..default()
        },
        children![(
            Text::new(text),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                margin: UiRect::all(px(50)),
                ..default()
            },
        )],
    ));
}
