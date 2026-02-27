use bevy::{input::common_conditions::input_toggle_active, prelude::*};
use bevy_inspector_egui::{
    bevy_egui::EguiPlugin,
    quick::{StateInspectorPlugin, WorldInspectorPlugin},
};

use crate::GameState;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        EguiPlugin::default(),
        StateInspectorPlugin::<GameState>::default().run_if(toggle_inspector()),
        WorldInspectorPlugin::default().run_if(toggle_inspector()),
    ));
}

pub fn toggle_inspector() -> impl FnMut(Res<ButtonInput<KeyCode>>) -> bool + Clone {
    input_toggle_active(false, KeyCode::F1)
}
