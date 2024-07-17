// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

#![allow(dead_code)]

//! Contains default implementations for the platform specific code, that can be (partially-)reused
//! on platforms that don't need customization.  Only accessed by the platform specific modules
//! below, so it not exposed as public.

use bevy::app::App;

/// Does nothing on platforms which don't need customization.
pub(crate) fn tweak_bevy_app(app: &mut App) {
    let _ = app;
}

pub(crate) mod menubar {
    use crate::menu;
    use bevy::winit::WinitWindows;

    // Currently does nothing, and is present merely to ensure we compile on
    // platforms, including those that don't natively support any menubar
    // functionality.
    pub(crate) fn attach_menu(windows: &WinitWindows, blueprint: &menu::Blueprint) {
        let _ = (windows, blueprint);
    }
}

// End of File
