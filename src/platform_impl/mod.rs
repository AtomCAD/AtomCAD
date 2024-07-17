// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at <http://mozilla.org/MPL/2.0/>.

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
    target_family = "wasm",
))]
mod defaults;
#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
))]
pub(crate) use defaults::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;

#[cfg(target_family = "wasm")]
mod web;
#[cfg(target_family = "wasm")]
pub(crate) use web::*;

// End of File
