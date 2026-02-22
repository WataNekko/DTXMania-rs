use bevy::prelude::*;
use dtxmania_rs::AppPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins((DefaultPlugins, AppPlugin))
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
