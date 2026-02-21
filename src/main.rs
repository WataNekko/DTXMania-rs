use std::{env, path::PathBuf};

use async_channel::Receiver;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, futures_lite::StreamExt},
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .insert_state(SongScanState::Scanning)
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::SongSelect), song_select_setup)
        .add_systems(OnEnter(SongScanState::Scanning), song_scan_start)
        .add_systems(
            Update,
            update_song_list.run_if(in_state(SongScanState::Scanning)),
        )
        .add_systems(OnExit(SongScanState::Scanning), song_scan_cleanup)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    SongSelect,
    Gameplay,
}

#[derive(Component)]
struct SongList;

const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

fn song_select_setup(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(GameState::SongSelect),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::End,
            padding: UiRect::right(Val::Px(50.)),
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.3, 0.3, 0.1)),
            children![
                (
                    Text::new("Song Select"),
                    TextFont {
                        font_size: 67.0,
                        ..default()
                    },
                    TextColor(TEXT_COLOR),
                    Node {
                        margin: UiRect::all(px(50)),
                        ..default()
                    },
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    SongList,
                )
            ]
        )],
    ));
}

fn get_base_path() -> PathBuf {
    #[cfg(feature = "dev")]
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        return PathBuf::from(manifest_dir);
    }

    env::current_exe()
        .map(|path| path.parent().map(ToOwned::to_owned).unwrap())
        .unwrap()
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum SongScanState {
    Scanning,
    Done,
}

#[derive(Resource, Deref)]
struct SongScanChannel(Receiver<Vec<PathBuf>>);

fn song_scan_start(mut commands: Commands) {
    info!("Starting song scan");
    let (tx, rx) = async_channel::bounded(1);
    commands.insert_resource(SongScanChannel(rx));

    IoTaskPool::get()
        .spawn(async move {
            let mut songs = Vec::new();
            let path = {
                let mut p = get_base_path();
                p.push("DTXFiles");
                p
            };
            scan_dir_recursive(path, &mut songs).await;

            tx.send(songs)
                .await
                .expect("Channel should still be opened during song scan");
        })
        .detach();

    async fn scan_dir_recursive(path: PathBuf, songs: &mut Vec<PathBuf>) {
        match async_fs::read_dir(path.as_path()).await {
            Err(err) => error!("Failed to read dir {}: {}", path.display(), err),
            Ok(entries) => {
                let mut entries = entries.filter_map(|res| {
                    res.inspect_err(|err| error!("Error scanning dir {}: {}", path.display(), err))
                        .ok()
                });

                while let Some(entry) = entries.next().await {
                    let path = entry.path();
                    let metadata = match async_fs::metadata(path.as_path()).await {
                        Err(err) => {
                            error!(
                                "Error reading metadata {}: {}",
                                entry.file_name().display(),
                                err
                            );
                            continue;
                        }
                        Ok(m) => m,
                    };

                    if metadata.is_dir() {
                        Box::pin(scan_dir_recursive(path, songs)).await;
                        continue;
                    }

                    let name = entry.file_name();

                    const DTX_EXTENSION: &[u8] = b".dtx";
                    const DTX_EXTENSION_LEN: usize = DTX_EXTENSION.len();
                    if name
                        .as_encoded_bytes()
                        .last_chunk::<DTX_EXTENSION_LEN>()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case(DTX_EXTENSION))
                    {
                        songs.push(path);
                    }
                }
            }
        }
    }
}

fn song_scan_cleanup(mut commands: Commands) {
    commands.remove_resource::<SongScanChannel>();
    info!("Song scan completed");
}

fn update_song_list(
    mut commands: Commands,
    song_list: Single<Entity, With<SongList>>,
    song_scan: Res<SongScanChannel>,
    mut next_state: ResMut<NextState<SongScanState>>,
) {
    let Ok(songs) = song_scan.try_recv() else {
        return;
    };
    next_state.set(SongScanState::Done);

    for song in songs {
        let Some(name) = song.file_name().map(|name| name.to_string_lossy()) else {
            continue;
        };

        let child = commands
            .spawn((
                Node {
                    width: px(300),
                    height: px(65),
                    margin: UiRect::all(px(2)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                children![(
                    Text::new(name),
                    TextFont {
                        font_size: 33.0,
                        ..default()
                    },
                    TextColor(TEXT_COLOR),
                ),],
            ))
            .id();

        commands.entity(*song_list).add_child(child);
    }
}
