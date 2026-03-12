use std::{collections::HashMap, io};

use async_fs::File;
use bevy::{
    input::common_conditions::input_just_pressed,
    prelude::*,
    tasks::{IoTaskPool, Task, futures::check_ready, futures_lite::io::BufReader},
    time::Stopwatch,
};

use crate::{
    GameState,
    song::{Channel, DtxChart, SongDatabase, SongPlaying, parse_dtx_chart},
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
            .add_systems(
                OnEnter(GameplayState::Playing),
                (playing_setup, spawn_chips).chain(),
            )
            .add_systems(
                PreUpdate,
                sync_playback_time.run_if(in_state(GameplayState::Playing)),
            )
            .add_systems(
                Update,
                (
                    update_chips_pos,
                    (|mut commands: Commands| commands.trigger(Return))
                        .run_if(input_just_pressed(KeyCode::Backquote)),
                )
                    .run_if(in_state(GameplayState::Playing)),
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

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ChipLaneContainer {
    channel: Channel,
}

fn playing_setup(mut commands: Commands, song: Res<LoadedSong>) {
    let title = song.chart.title.clone();
    let lanes = [
        Channel::HiHatClose,
        Channel::Snare,
        Channel::BassDrum,
        Channel::HighTom,
        Channel::LowTom,
        Channel::Cymbal,
        Channel::FloorTom,
        Channel::HiHatOpen,
        Channel::RideCymbal,
        Channel::LeftCymbal,
        Channel::LeftPedal,
        Channel::LeftBass,
    ];

    commands.spawn((
        DespawnOnExit(GameplayState::Playing),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(50)),
            row_gap: px(50),
            ..default()
        },
        children![
            (Text::new(title)),
            (
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::End,
                    column_gap: px(14),
                    padding: UiRect::all(px(16)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 0.0, 0.1)),
                Children::spawn(SpawnIter(lanes.into_iter().map(|channel| {
                    (
                        Node {
                            width: px(54),
                            height: percent(100),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(30),
                            ..default()
                        },
                        children![
                            (Text::new({
                                let mut ch = channel.to_string();
                                ch.retain(|c| c.is_uppercase());
                                ch
                            })),
                            (
                                Node {
                                    width: percent(100),
                                    height: percent(100),
                                    margin: UiRect {
                                        left: px(5),
                                        right: px(5),
                                        ..default()
                                    },
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.1, 0.0, 0.0)),
                            ),
                            (
                                Name::new(format!("{} lane", channel)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    bottom: px(0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                ChipLaneContainer { channel },
                            )
                        ],
                    )
                })))
            )
        ],
    ));
}

#[derive(Component, Deref)]
struct ChipTime(f64);

#[derive(Resource, Deref, DerefMut)]
struct PlaybackTime(Stopwatch);

fn spawn_chips(
    lane_query: Query<(&ChipLaneContainer, Entity)>,
    mut commands: Commands,
    song: Res<LoadedSong>,
) {
    let lanes: HashMap<_, _> = lane_query
        .into_iter()
        .map(|(lane, entity)| (lane.channel, entity))
        .collect();

    for chip in &song.chart.chips {
        if let Some(&lane) = lanes.get(&chip.channel) {
            commands.entity(lane).with_children(|parent| {
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(54),
                        height: px(10),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.8, 0.8, 0.0)),
                    ChipTime(chip.time_sec),
                ));
            });
        }
    }

    commands.insert_resource(PlaybackTime(Stopwatch::new()));
}

fn sync_playback_time(time: Res<Time>, mut playback_time: ResMut<PlaybackTime>) {
    playback_time.tick(time.delta());
}

fn update_chips_pos(chip_query: Query<(&ChipTime, &mut UiTransform)>, time: Res<PlaybackTime>) {
    for (ChipTime(chip_time), mut transform) in chip_query {
        const SCROLL_SPEED: f64 = 800.0;
        transform.translation.y = px((time.elapsed_secs_f64() - chip_time) * SCROLL_SPEED);
    }
}
