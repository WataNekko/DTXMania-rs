use std::collections::HashMap;

use bevy::{input::common_conditions::input_just_pressed, prelude::*, time::Stopwatch};

use crate::{
    gameplay::{GameplayState, LoadedSong, Return},
    song::Channel,
};

pub struct PlayingPlugin;

impl Plugin for PlayingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameplayState::Playing),
            (setup, spawn_chips).chain(),
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
        );
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ChipLaneContainer {
    channel: Channel,
}

#[derive(Component, Deref)]
struct ChipTime(f64);

#[derive(Resource, Deref, DerefMut)]
struct PlaybackTime(Stopwatch);

fn setup(mut commands: Commands, song: Res<LoadedSong>) {
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
