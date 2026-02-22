use bevy::prelude::*;

use crate::song::SongDatabase;

pub struct SongSelectPlugin<S> {
    pub on_state: S,
}

impl<S: States + Copy> Plugin for SongSelectPlugin<S> {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(self.on_state), song_select_setup(self.on_state))
            .add_systems(
                Update,
                refresh_songs_container.run_if(resource_exists_and_changed::<SongDatabase>),
            );
    }
}

#[derive(Component)]
struct SongsContainer;

const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

fn song_select_setup(state: impl States + Copy) -> impl Fn(Commands) {
    move |mut commands| {
        commands.spawn((
            DespawnOnExit(state),
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
                        SongsContainer,
                    )
                ]
            )],
        ));
    }
}

fn refresh_songs_container(
    container: Single<Entity, With<SongsContainer>>,
    mut commands: Commands,
    song_db: Res<SongDatabase>,
) {
    commands
        .entity(*container)
        .despawn_children()
        .with_children(|parent| {
            for song in song_db.iter() {
                let Some(name) = song.file_name().map(|name| name.to_string_lossy()) else {
                    continue;
                };

                parent.spawn((
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
                ));
            }
        });
}
