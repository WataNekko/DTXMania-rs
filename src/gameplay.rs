use std::{io, time::Duration};

use async_fs::File;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready, futures_lite::io::BufReader},
    time::common_conditions::on_timer,
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
            .add_systems(OnEnter(GameplayState::Loading), load_song)
            .add_systems(
                Update,
                handle_song_load_done.run_if(in_state(GameplayState::Loading)),
            )
            .add_systems(OnEnter(GameplayState::Play), play_setup)
            .add_systems(
                Update,
                (|mut commands: Commands| commands.trigger(Return))
                    .run_if(in_state(GameplayState::Play).and(on_timer(Duration::from_secs(5)))),
            )
            .add_observer(on_return);

        #[cfg(feature = "dev")]
        {
            use crate::debug::toggle_inspector;
            use bevy_inspector_egui::quick::StateInspectorPlugin;

            app.add_plugins(
                StateInspectorPlugin::<GameplayState>::default()
                    .run_if(toggle_inspector().and(in_state(GameState::Gameplay))),
            );
        }
    }
}

#[derive(States, Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Reflect)]
enum GameplayState {
    Loading,
    Play,
    #[default]
    Disabled,
}

#[derive(Resource)]
struct SongLoadTask(Task<io::Result<DtxChart>>);

#[derive(Resource)]
struct LoadedSong(DtxChart);

#[derive(Event)]
struct Return;

fn setup(mut state: ResMut<NextState<GameplayState>>) {
    state.set(GameplayState::Loading);
}

fn load_song(mut commands: Commands, song_playing: Res<SongPlaying>, song_db: Res<SongDatabase>) {
    let song = song_db[song_playing.db_idx].clone();
    info!("Loading song: {}", song.display());

    let task = IoTaskPool::get().spawn(async move {
        let file = File::open(song).await?;
        let reader = BufReader::new(file);
        parse_dtx_chart(reader).await
    });

    commands.insert_resource(SongLoadTask(task));
}

fn handle_song_load_done(
    mut commands: Commands,
    mut song_load: ResMut<SongLoadTask>,
    mut next_state: ResMut<NextState<GameplayState>>,
) {
    let Some(res) = check_ready(&mut song_load.0) else {
        return;
    };
    commands.remove_resource::<SongLoadTask>();

    match res {
        Err(err) => {
            error!("Error loading song: {}", err);
            commands.trigger(Return);
        }
        Ok(chart) => {
            commands.insert_resource(LoadedSong(chart));
            next_state.set(GameplayState::Play);
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

fn play_setup(mut commands: Commands, song: Res<LoadedSong>) {
    let text = song.0.title.clone();

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
