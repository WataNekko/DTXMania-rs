use std::collections::HashMap;

use bevy::{
    input::{common_conditions::input_just_pressed, keyboard::Key},
    prelude::*,
};
use bevy_seedling::{
    firewheel::clock::{DurationSeconds, InstantSeconds},
    sample::SamplePlayer,
    time::{Audio, AudioTime},
};

use crate::{
    assets::song::DrumNote,
    gameplay::{GameplayState, LoadedSong, Return},
};

pub struct PlayingPlugin;

impl Plugin for PlayingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameplayState::Playing),
            ((setup_ui, spawn_chips).chain(), start_playback),
        )
        .add_systems(OnExit(GameplayState::Playing), cleanup_playback)
        .add_systems(
            PreUpdate,
            update_playback_time.run_if(in_state(GameplayState::Playing)),
        )
        .add_systems(
            Update,
            (
                update_chips_pos,
                (|mut commands: Commands| commands.trigger(Return))
                    .run_if(input_just_pressed(Key::Escape)),
                play_bgm_chips,
                autoplay_drum_chips,
            )
                .run_if(in_state(GameplayState::Playing)),
        );
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ChipLaneContainer {
    lane_note: DrumNote,
}

#[derive(Component, Deref)]
struct ChipTime(DurationSeconds);

#[derive(Resource, Deref)]
struct PlaybackStart(InstantSeconds);

#[derive(Resource, Deref, DerefMut)]
struct PlaybackTime(DurationSeconds);

#[derive(Resource, Default)]
struct NextBgmChipIdx(usize);

#[derive(Resource, Default, Deref, DerefMut)]
struct NextDrumChipsIndices(HashMap<DrumNote, usize>);

fn setup_ui(mut commands: Commands, song: Res<LoadedSong>) {
    let title = song.chart.title.clone();
    let lanes = [
        DrumNote::HiHatClose,
        DrumNote::Snare,
        DrumNote::BassDrum,
        DrumNote::HighTom,
        DrumNote::LowTom,
        DrumNote::Cymbal,
        DrumNote::FloorTom,
        DrumNote::HiHatOpen,
        DrumNote::RideCymbal,
        DrumNote::LeftCymbal,
        DrumNote::LeftPedal,
        DrumNote::LeftBass,
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
                Children::spawn(SpawnIter(lanes.into_iter().map(|lane_note| {
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
                                let mut lane = format!("{:?}", lane_note);
                                lane.retain(|c| c.is_uppercase());
                                lane
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
                                Name::new(format!("{:?} lane", lane_note)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    bottom: px(0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                ChipLaneContainer { lane_note },
                            )
                        ],
                    )
                })))
            )
        ],
    ));
}

fn spawn_chips(
    lane_query: Query<(&ChipLaneContainer, Entity)>,
    mut commands: Commands,
    song: Res<LoadedSong>,
) {
    let lanes: HashMap<_, _> = lane_query
        .into_iter()
        .map(|(lane, entity)| (lane.lane_note, entity))
        .collect();

    for (note, chips) in &song.chart.drum_chips {
        let Some(&lane) = lanes.get(note) else {
            continue;
        };

        commands.entity(lane).with_children(|parent| {
            for chip in chips {
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(54),
                        height: px(10),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.8, 0.8, 0.0)),
                    ChipTime(chip.time),
                ));
            }
        });
    }
}

fn start_playback(mut commands: Commands, time: Res<Time<Audio>>) {
    commands.insert_resource(NextBgmChipIdx(0));
    commands.insert_resource(NextDrumChipsIndices(HashMap::new()));
    commands.insert_resource(PlaybackStart(time.now()));
    commands.insert_resource(PlaybackTime(DurationSeconds::ZERO));
}

fn cleanup_playback(mut commands: Commands) {
    commands.remove_resource::<NextBgmChipIdx>();
    commands.remove_resource::<NextDrumChipsIndices>();
    commands.remove_resource::<PlaybackStart>();
    commands.remove_resource::<PlaybackTime>();
}

fn update_playback_time(
    time: Res<Time<Audio>>,
    playback_start: Res<PlaybackStart>,
    mut playback_time: ResMut<PlaybackTime>,
) {
    **playback_time = time.now() - **playback_start;
}

fn update_chips_pos(chip_query: Query<(&ChipTime, &mut UiTransform)>, time: Res<PlaybackTime>) {
    for (ChipTime(chip_time), mut transform) in chip_query {
        const SCROLL_SPEED: f64 = 800.0;
        transform.translation.y = px((time.0.0 - chip_time.0) * SCROLL_SPEED);
    }
}

fn play_bgm_chips(
    mut commands: Commands,
    song: Res<LoadedSong>,
    mut next_idx: ResMut<NextBgmChipIdx>,
    time: Res<PlaybackTime>,
) {
    let played_count = song.chart.bgm_chips[next_idx.0..]
        .iter()
        .take_while(|chip| chip.time <= **time)
        .inspect(|chip| {
            commands.spawn((
                DespawnOnExit(GameplayState::Playing),
                SamplePlayer::new(chip.audio.clone()),
            ));
        })
        .count();

    next_idx.0 += played_count;
}

fn autoplay_drum_chips(
    mut commands: Commands,
    song: Res<LoadedSong>,
    mut next_indices: ResMut<NextDrumChipsIndices>,
    time: Res<PlaybackTime>,
) {
    for (note, chips) in &song.chart.drum_chips {
        let next_idx = next_indices.entry(*note).or_default();

        let played_count = chips[*next_idx..]
            .iter()
            .take_while(|chip| chip.time <= **time)
            .inspect(|chip| {
                commands.spawn((
                    DespawnOnExit(GameplayState::Playing),
                    SamplePlayer::new(chip.audio.clone()),
                ));
            })
            .count();

        *next_idx += played_count;
    }
}
