use std::io;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready},
};

use crate::{
    gameplay::{GameplayState, LoadedSong, Return},
    song::{SongDatabase, SongPlaying, load_dtx_chart},
};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameplayState::Loading), (setup, load_song))
            .add_systems(
                Update,
                handle_song_load_result.run_if(resource_exists::<SongLoadTask>),
            )
            .add_systems(Update, countdown.run_if(in_state(GameplayState::Loading)));
    }
}

#[derive(Resource, Deref, DerefMut)]
struct LoadingTimer(Timer);

#[derive(Resource)]
struct SongLoadTask(Task<io::Result<LoadedSong>>);

fn setup(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(GameplayState::Loading),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::right(px(50)),
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
        load_dtx_chart(song_path)
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

fn countdown(
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
