use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::{
    GameState,
    assets::song::{SongDatabase, SongPlaying},
};

pub struct SongSelectPlugin;

impl Plugin for SongSelectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::SongSelect),
            (
                song_select_setup,
                refresh_songs_container.run_if(resource_exists::<SongDatabase>),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                refresh_songs_container.run_if(resource_exists_and_changed::<SongDatabase>),
                (
                    navigate(-1).run_if(input_just_pressed(KeyCode::ArrowUp)),
                    navigate(1).run_if(input_just_pressed(KeyCode::ArrowDown)),
                    confirm_selection.run_if(
                        resource_exists::<SelectedSongIndex>
                            .and(input_just_pressed(KeyCode::Enter)),
                    ),
                ),
                focus_selected.run_if(resource_exists_and_changed::<SelectedSongIndex>),
            )
                .chain()
                .run_if(in_state(GameState::SongSelect)),
        );
    }
}

#[derive(Component)]
struct SongsContainer;

#[derive(Resource)]
struct SelectedSongIndex(usize);

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
                    SongsContainer,
                )
            ]
        )],
    ));
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
                let name = song
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .expect("song_db should contain only valid parsed entries");

                parent.spawn((
                    Node {
                        width: px(300),
                        height: px(65),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(5)),
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

    commands.remove_resource::<SelectedSongIndex>();
}

fn navigate(
    step: isize,
) -> impl Fn(Single<&Children, With<SongsContainer>>, Commands, Option<ResMut<SelectedSongIndex>>) {
    move |songs, mut commands, selected| {
        if songs.is_empty() {
            return;
        }

        let selected = selected.map(|s| s.0).unwrap_or(0);
        commands.entity(songs[selected]).remove::<BorderColor>();

        let new_selected = (selected as isize + step).rem_euclid(songs.len() as _) as usize;
        commands
            .entity(songs[new_selected])
            .insert(BorderColor::all(Color::srgb(0.8, 0.0, 0.0)));
        commands.insert_resource(SelectedSongIndex(new_selected));
    }
}

fn focus_selected(
    container: Single<(&mut UiTransform, &Children), With<SongsContainer>>,
    node_query: Query<&ComputedNode, Without<SongsContainer>>,
    selected: Res<SelectedSongIndex>,
) {
    let (mut transform, children) = container.into_inner();
    let Some(node) = children.first().and_then(|&e| node_query.get(e).ok()) else {
        return;
    };
    transform.translation.y = -Val::Px(selected.0 as f32 * node.size.y);
}

fn confirm_selection(
    mut commands: Commands,
    idx: Res<SelectedSongIndex>,
    song_db: Res<SongDatabase>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    info!("Selected song: {}", song_db[idx.0].display());
    commands.insert_resource(SongPlaying { db_idx: idx.0 });
    next_state.set(GameState::Gameplay);
}
