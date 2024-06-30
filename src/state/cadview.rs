// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

use crate::{AppState, FontAssets};
use bevy::app::App;
use bevy::prelude::*;

pub struct CadViewPlugin;

impl Plugin for CadViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::CadView), setup_cad_view)
            .add_systems(OnExit(AppState::CadView), cleanup_cad_view);
    }
}

// Tag component used to tag entities added on in CAD view.
#[derive(Component)]
struct OnCadView;

fn setup_cad_view(mut commands: Commands, font_assets: Res<FontAssets>) {
    commands.spawn((Camera2dBundle::default(), OnCadView));
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ..default()
            },
            OnCadView,
        ))
        .with_children(|parent| {
            parent.spawn(
                TextBundle::from_section(
                    "Hello, world!",
                    TextStyle {
                        font: font_assets.fira_sans_bold.clone(),
                        font_size: 64.0,
                        color: Color::WHITE,
                    },
                )
                .with_style(Style {
                    margin: UiRect::all(Val::Auto),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                }),
            );
        });
}

fn cleanup_cad_view(mut commands: Commands, entities: Query<Entity, With<OnCadView>>) {
    for entity in entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// End of File