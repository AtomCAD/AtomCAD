// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

use objc::rc::autoreleasepool;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use crate::keyboard::ModifierKeys;
use crate::menu::{self, SystemAction, SystemShortcut};

use bevy::winit::WinitWindows;

fn nsstring(s: &str) -> *mut Object {
    unsafe {
        let cls = class!(NSString);
        let bytes = s.as_ptr();
        let len = s.len();
        let encoding = 4; // UTF-8
        let obj: *mut Object = msg_send![cls, alloc];
        let obj: *mut Object = msg_send![obj, initWithBytes:bytes length:len encoding:encoding];
        let obj: *mut Object = msg_send![obj, autorelease];
        obj
    }
}

unsafe fn build_menu(services_menu: *mut Object, blueprint: &menu::Blueprint) -> *mut Object {
    // Create root menu bar.
    let menuobj: *mut Object = msg_send![class![NSMenu], alloc];
    let menuobj: *mut Object = msg_send![menuobj, initWithTitle: nsstring(&blueprint.title)];
    let menuobj: *mut Object = msg_send![menuobj, autorelease];

    for menuitem in blueprint.items.iter() {
        match menuitem {
            menu::Item::Separator => {
                let item: *mut Object = msg_send![class![NSMenuItem], separatorItem];
                let _: () = msg_send![menuobj, addItem: item];
            }
            menu::Item::Entry {
                title,
                shortcut,
                action,
            } => {
                let title = nsstring(title);
                let mut is_service_menu = false;
                let action = match action {
                    menu::Action::System(action) => match action {
                        SystemAction::LaunchAboutWindow => {
                            Some(sel!(orderFrontStandardAboutPanel:))
                        }
                        SystemAction::LaunchPreferences => Some(sel!(orderFrontPreferencesPanel:)),
                        SystemAction::ServicesMenu => {
                            is_service_menu = true;
                            None
                        }
                        SystemAction::HideApp => Some(sel!(hide:)),
                        SystemAction::HideOthers => Some(sel!(hideOtherApplications:)),
                        SystemAction::ShowAll => Some(sel!(unhideAllApplications:)),
                        SystemAction::Terminate => Some(sel!(terminate:)),
                    },
                };
                let shortcutkey = match shortcut {
                    menu::Shortcut::None => nsstring(""),
                    menu::Shortcut::System(shortcut) => match shortcut {
                        SystemShortcut::Preferences => nsstring(","),
                        SystemShortcut::HideApp => nsstring("h"),
                        SystemShortcut::HideOthers => nsstring("h"),
                        SystemShortcut::QuitApp => nsstring("q"),
                    },
                };
                let shotcutmodifiers = match shortcut {
                    menu::Shortcut::None => ModifierKeys::empty(),
                    menu::Shortcut::System(shortcut) => match shortcut {
                        SystemShortcut::Preferences => ModifierKeys::COMMAND,
                        SystemShortcut::HideApp => ModifierKeys::COMMAND,
                        SystemShortcut::HideOthers => ModifierKeys::COMMAND | ModifierKeys::OPTION,
                        SystemShortcut::QuitApp => ModifierKeys::COMMAND,
                    },
                };
                let mut item: *mut Object = msg_send![class![NSMenuItem], alloc];
                if let Some(action) = action {
                    item = msg_send![item,
                                     initWithTitle: title
                                     action: action
                                     keyEquivalent: shortcutkey];
                } else {
                    item = msg_send![item,
                                     initWithTitle: title
                                     action: 0
                                     keyEquivalent: shortcutkey];
                }
                if shotcutmodifiers != ModifierKeys::empty() {
                    let mut modifiermask = 0usize;
                    if shotcutmodifiers.contains(ModifierKeys::CAPSLOCK) {
                        modifiermask |= 1 << 16; // NSEventModifierFlagCapsLock
                    }
                    if shotcutmodifiers.contains(ModifierKeys::SHIFT) {
                        modifiermask |= 1 << 17; // NSEventModifierFlagShift
                    }
                    if shotcutmodifiers.contains(ModifierKeys::CONTROL) {
                        modifiermask |= 1 << 18; // NSEventModifierFlagControl
                    }
                    if shotcutmodifiers.contains(ModifierKeys::OPTION) {
                        modifiermask |= 1 << 19; // NSEventModifierFlagOption
                    }
                    if shotcutmodifiers.contains(ModifierKeys::COMMAND) {
                        modifiermask |= 1 << 20; // NSEventModifierFlagCommand
                    }
                    if shotcutmodifiers.contains(ModifierKeys::NUMPAD) {
                        modifiermask |= 1 << 21; // NSEventModifierFlagNumericPad
                    }
                    if shotcutmodifiers.contains(ModifierKeys::HELP) {
                        modifiermask |= 1 << 22; // NSEventModifierFlagHelp
                    }
                    if shotcutmodifiers.contains(ModifierKeys::FUNCTION) {
                        modifiermask |= 1 << 23; // NSEventModifierFlagFunction
                    }
                    let _: () = msg_send![item, setKeyEquivalentModifierMask: modifiermask];
                }
                item = msg_send![item, autorelease];
                if is_service_menu {
                    let _: () = msg_send![item, setSubmenu: services_menu];
                }
                let _: () = msg_send![menuobj, addItem: item];
            }
            menu::Item::SubMenu(submenu) => {
                let item: *mut Object = msg_send![class![NSMenuItem], alloc];
                let item: *mut Object = msg_send![item, init];
                let item: *mut Object = msg_send![item, autorelease];
                let submenu = build_menu(services_menu, submenu);
                let _: () = msg_send![item, setSubmenu: submenu];
                let _: () = msg_send![menuobj, addItem: item];
            }
        }
    }

    // Return the menu object to the caller.
    menuobj
}

pub(crate) fn attach_menu(
    // On some platforms, e.g. Windows and Linux, the menu bar is part of the window itself, and we
    // need to add it to each individual window.  But for macOS the menu bar is a property of the
    // NSApplication instance shared by the entire process, so we only need to set it once and don't
    // use the `WinitWindows` parameter.
    windows: &WinitWindows,
    blueprint: &menu::Blueprint,
) {
    let _ = windows; // unused on macOS

    // Create the menu on macOS using Cocoa APIs.
    #[cfg(target_os = "macos")]
    autoreleasepool(|| unsafe {
        // Get the application object.
        let app: *mut Object = msg_send![class![NSApplication], sharedApplication];

        // Create and register the services menu.
        let services_menu: *mut Object = msg_send![class![NSMenu], alloc];
        let services_menu: *mut Object = msg_send![services_menu, init];
        let services_menu: *mut Object = msg_send![services_menu, autorelease];
        let _: () = msg_send![app, setServicesMenu: services_menu];

        // Turn the menubar description into a Cocoa menu.
        let obj = build_menu(services_menu, blueprint);

        // Register the menu with the NSApplication object.
        let _: () = msg_send![app, setMainMenu: obj];
    });
}

// End of File
