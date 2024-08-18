// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

// Bevy uses some very complex types for specifying system inputs.
// There's just no getting around this, so silence clippy's complaints.
#![allow(clippy::type_complexity)]

pub mod platform;
pub(crate) mod platform_impl;

pub mod assets;
use assets::FontAssets;

pub mod gui;
use gui::set_window_icon;

pub mod keyboard;

pub mod menu;
use menu::MenuBarPlugin;

pub mod state;
use state::cadview::CadViewPlugin;
use state::loading::LoadingPlugin;
use state::splashscreen::SplashScreenPlugin;

use bevy::app::App;
use bevy::prelude::*;

pub const APP_NAME: &str = "atomCAD";

// We use States to separate logic
// See https://bevy-cheatbook.github.io/programming/states.html
// Or https://github.com/bevyengine/bevy/blob/main/examples/ecs/state.rs
#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
enum AppState {
    // During the loading State the LoadingPlugin will load our assets
    #[default]
    Loading,
    // Here the “Get Started” prompt is drawn and we wait for user interaction.
    SplashScreen,
    // During this State the scene graph is rendered and the user can interact
    // with the camera.
    CadView,
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_plugins((
                MenuBarPlugin,
                LoadingPlugin,
                SplashScreenPlugin,
                CadViewPlugin,
            ))
            .add_systems(Startup, set_window_icon);
    }
}

// End of File
