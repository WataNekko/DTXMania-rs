use bevy::{asset::UnapprovedPathMode, prelude::*};
use bevy_seedling::SeedlingPlugin;
use dtxmania_rs::{AppPlugin, DtxAssetPlugin};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DtxAssetPlugin,
            DefaultPlugins.set(AssetPlugin {
                // DTXMania may read charts and files from arbitrary locations set by DTXPath in
                // Config.ini. This design fundamentally implies security risks.
                //
                // Maybe data access can be restricted to within locations in DTXPath only (using
                // custom AssetSources). But if we don't trust paths from user charts, then should
                // we trust paths from user config?
                //
                // For simplicity, we'll allow all paths for now.
                unapproved_path_mode: UnapprovedPathMode::Allow,
                ..default()
            }),
            SeedlingPlugin::default(),
            AppPlugin,
        ))
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
