use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    input::common_conditions::input_toggle_active,
    prelude::*,
};
use bevy_inspector_egui::{
    bevy_egui::EguiPlugin,
    quick::{StateInspectorPlugin, WorldInspectorPlugin},
};

use crate::GameState;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        EguiPlugin::default(),
        WorldInspectorPlugin::default().run_if(inspector_toggled()),
        StateInspectorPlugin::<GameState>::default().run_if(inspector_toggled()),
        FpsOverlayPlugin {
            config: FpsOverlayConfig {
                enabled: false,
                frame_time_graph_config: FrameTimeGraphConfig {
                    enabled: false,
                    ..default()
                },
                ..default()
            },
        },
    ))
    .add_systems(Update, toggle_fps_overlay);
}

pub fn inspector_toggled() -> impl FnMut(Res<ButtonInput<KeyCode>>) -> bool + Clone {
    input_toggle_active(false, KeyCode::F1)
}

fn toggle_fps_overlay(input: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<FpsOverlayConfig>) {
    if input.just_pressed(KeyCode::F2) {
        if input.pressed(KeyCode::ShiftLeft) || input.pressed(KeyCode::ShiftRight) {
            overlay.frame_time_graph_config.enabled = !overlay.frame_time_graph_config.enabled;
        } else {
            overlay.enabled = !overlay.enabled;
        }
    }
}
