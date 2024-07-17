// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

use crate::{platform::menubar::attach_menu, APP_NAME};
use bevy::{prelude::*, winit::WinitWindows};

/// This plugin is responsible for the initializing the application's menu bar, attaching it to the
/// primary window, and handling any keyboard shortcut or menu selection events.
pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        // Setup the menu bar, and attach it to the primary window (on Windows or X11), or to the
        // application itself (macOS).
        app.add_systems(Startup, setup_menu_bar);
    }
}

/// A menubar is a hierarchical list of actions with attached titles and/or keyboard shortcuts.  It
/// is attached to either the application instance (macOS), the main window (Windows/Linux), or
/// fully emulated (mobile/web).  On platforms that lack per-window menubars, the application must
/// switch the global menubar based on the active window.
///
/// Menus can also be contextual (e.g. a popup right-click menu) or accessed from the system tray.
pub struct Blueprint {
    pub title: String,
    pub items: Vec<Item>,
}

impl Blueprint {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            items: Vec::new(),
        }
    }
}

/// A menu item is either an action (with an optional keyboard shortcut) or a submenu.  The
/// [`Separator`](Item::Separator) is a visual divider between groups of related menu items.
pub enum Item {
    Separator,
    Entry {
        title: String,
        shortcut: Shortcut,
        action: Action,
    },
    SubMenu(Blueprint),
}

/// A keyboard shortcut is a combination of modifier keys (e.g. Shift, Option, Alt, etc.) and the
/// key to press (indicated by a unicode character).  Some shortcuts for common actions like copy,
/// paste, quit, etc. are system-wide and cannot be overridden by the application.
#[derive(Clone, Copy)]
pub enum Shortcut {
    None,
    System(SystemShortcut),
}

/// Common actions like copy-paste, file-open, and quit are usually bound to shortcuts that vary
/// from platform to platform, but are expected to remain consistent across all apps on that
/// platform.
#[derive(Clone, Copy)]
pub enum SystemShortcut {
    Preferences,
    HideApp,
    HideOthers,
    QuitApp,
}

/// A menu action is a callback that is invoked when the menu item is selected.  It can be either an
/// internal, application-defined action, or a system response implemented by the operating system.
pub enum Action {
    System(SystemAction),
}

/// System actions are predefined actions that are implemented by the operating system.  They are
/// usually used for common actions like showing the preferences window, hiding the app, etc.
pub enum SystemAction {
    LaunchAboutWindow,
    LaunchPreferences,
    ServicesMenu,
    HideApp,
    HideOthers,
    ShowAll,
    Terminate,
}

pub fn setup_menu_bar(
    // We have to use `NonSend` here.  This forces this function to be called from the winit thread
    // (which is the main thread on macOS), and after the window has been created.  We don't
    // actually use the window on macOS, but we do need to be in the main (event loop) thread in
    // order to access the macOS APIs we need.
    windows: NonSend<WinitWindows>,
) {
    let blueprint = Blueprint {
        title: APP_NAME.into(),
        items: vec![Item::SubMenu(Blueprint {
            title: "".into(),
            items: vec![
                Item::Entry {
                    title: format!("About {}", APP_NAME),
                    shortcut: Shortcut::None,
                    action: Action::System(SystemAction::LaunchAboutWindow),
                },
                Item::Separator,
                Item::Entry {
                    title: "Settings...".into(),
                    shortcut: Shortcut::System(SystemShortcut::Preferences),
                    action: Action::System(SystemAction::LaunchPreferences),
                },
                Item::Separator,
                Item::Entry {
                    title: "Services".into(),
                    shortcut: Shortcut::None,
                    action: Action::System(SystemAction::ServicesMenu),
                },
                Item::Separator,
                Item::Entry {
                    title: format!("Hide {}", APP_NAME),
                    shortcut: Shortcut::System(SystemShortcut::HideApp),
                    action: Action::System(SystemAction::HideApp),
                },
                Item::Entry {
                    title: "Hide Others".into(),
                    shortcut: Shortcut::System(SystemShortcut::HideOthers),
                    action: Action::System(SystemAction::HideOthers),
                },
                Item::Entry {
                    title: "Show All".into(),
                    shortcut: Shortcut::None,
                    action: Action::System(SystemAction::ShowAll),
                },
                Item::Separator,
                Item::Entry {
                    title: format!("Quit {}", APP_NAME),
                    shortcut: Shortcut::System(SystemShortcut::QuitApp),
                    action: Action::System(SystemAction::Terminate),
                },
            ],
        })],
    };

    // Do the platform-dependent work of constructing the menubar and attaching it to the
    // application object or main window.
    attach_menu(&windows, &blueprint);
}

// End of File
