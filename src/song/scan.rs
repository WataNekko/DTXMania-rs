use std::{env, path::PathBuf};

use async_channel::Receiver;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, futures_lite::StreamExt},
};

use crate::song::SongDatabase;

pub struct SongScanPlugin;

impl Plugin for SongScanPlugin {
    fn build(&self, app: &mut App) {
        app.insert_state(SongScanState::Scanning)
            .add_systems(OnEnter(SongScanState::Scanning), song_scan_start)
            .add_systems(
                Update,
                handle_song_scan_message.run_if(in_state(SongScanState::Scanning)),
            )
            .add_systems(OnExit(SongScanState::Scanning), song_scan_cleanup);

        #[cfg(feature = "dev")]
        {
            use crate::debug::toggle_inspector;
            use bevy_inspector_egui::quick::StateInspectorPlugin;

            app.add_plugins(
                StateInspectorPlugin::<SongScanState>::default().run_if(toggle_inspector()),
            );
        }
    }
}

#[derive(States, Clone, Copy, Debug, Eq, PartialEq, Hash, Reflect)]
pub enum SongScanState {
    Scanning,
    Done,
}

#[derive(Resource, Deref)]
struct SongScanChannel(Receiver<Vec<PathBuf>>);

fn get_base_path() -> PathBuf {
    #[cfg(feature = "dev")]
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        return PathBuf::from(manifest_dir);
    }

    env::current_exe()
        .map(|path| path.parent().map(ToOwned::to_owned).unwrap())
        .unwrap()
}

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
}

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

fn song_scan_cleanup(mut commands: Commands) {
    commands.remove_resource::<SongScanChannel>();
    info!("Song scan completed");
}

fn handle_song_scan_message(
    mut commands: Commands,
    channel: Res<SongScanChannel>,
    mut next_state: ResMut<NextState<SongScanState>>,
) {
    if let Ok(songs) = channel.try_recv() {
        commands.insert_resource(SongDatabase(songs));
        next_state.set(SongScanState::Done);
    }
}
