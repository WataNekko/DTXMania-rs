use std::io;

use async_fs::File;
use bevy::{
    input::common_conditions::input_just_pressed,
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready, futures_lite::io::BufReader},
};

use crate::{
    GameState,
    song::{DtxChart, SongDatabase, SongPlaying, parse_dtx_chart},
};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameplayState>()
            .add_systems(OnEnter(GameState::Gameplay), setup)
            .add_systems(OnEnter(GameplayState::Loading), (loading_setup, load_song))
            .add_systems(
                Update,
                handle_song_load_result.run_if(resource_exists::<SongLoadTask>),
            )
            .add_systems(
                Update,
                loading_countdown.run_if(in_state(GameplayState::Loading)),
            )
            .add_systems(OnEnter(GameplayState::Playing), playing_setup)
            .add_systems(
                Update,
                (|mut commands: Commands| commands.trigger(Return)).run_if(
                    in_state(GameplayState::Playing).and(input_just_pressed(KeyCode::Backspace)),
                ),
            )
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

#[derive(Resource, Deref, DerefMut)]
struct LoadingTimer(Timer);

#[derive(Resource)]
struct SongLoadTask(Task<io::Result<LoadedSong>>);

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

fn loading_setup(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(GameplayState::Loading),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::right(Val::Px(50.)),
            ..default()
        },
        children![(
            Text::new("LOADING..."),
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

    commands.insert_resource(LoadingTimer(Timer::from_seconds(0.5, TimerMode::Once)));
}

fn load_song(
    mut commands: Commands,
    song_playing: Res<SongPlaying>,
    loaded_song: Option<Res<LoadedSong>>,
    song_db: Res<SongDatabase>,
) {
    let id = song_playing.db_idx;
    if loaded_song.is_some_and(|loaded| loaded.id == id) {
        return;
    }

    let song_path = song_db[id].clone();
    info!("Loading song: {}", song_path.display());

    let task = IoTaskPool::get().spawn(async move {
        let file = File::open(song_path).await?;
        let reader = BufReader::new(file);

        parse_dtx_chart(reader)
            .await
            .map(|chart| LoadedSong { id, chart })
    });

    commands.insert_resource(SongLoadTask(task));
}

fn handle_song_load_result(mut commands: Commands, mut task: ResMut<SongLoadTask>) {
    let Some(res) = check_ready(&mut task.0) else {
        return;
    };
    commands.remove_resource::<SongLoadTask>();

    match res {
        Err(err) => {
            error!("Error loading song: {}", err);
            commands.remove_resource::<LoadedSong>();
        }
        Ok(loaded_song) => {
            info!("Loaded song id: {}", loaded_song.id);
            commands.insert_resource(loaded_song);
        }
    }
}

fn loading_countdown(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<LoadingTimer>,
    load_task: Option<Res<SongLoadTask>>,
    loaded: Option<Res<LoadedSong>>,
    mut next_state: ResMut<NextState<GameplayState>>,
) {
    if timer.tick(time.delta()).is_finished() && load_task.is_none() {
        if loaded.is_some() {
            next_state.set(GameplayState::Playing);
        } else {
            commands.trigger(Return);
        }
    }
}

fn on_return(
    _: On<Return>,
    mut game_state: ResMut<NextState<GameState>>,
    mut play_state: ResMut<NextState<GameplayState>>,
) {
    game_state.set(GameState::SongSelect);
    play_state.set(GameplayState::Disabled);
}

fn playing_setup(mut commands: Commands, song: Res<LoadedSong>) {
    let text = song.chart.title.clone();

    commands.spawn((
        DespawnOnExit(GameplayState::Playing),
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
