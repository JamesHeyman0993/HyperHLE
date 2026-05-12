/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! GameKit framework.

use crate::dyld::{ConstantExports, HostConstant}; // Added for constants

pub mod ad_banner_view;
pub mod fb_session; 
pub mod gk_leaderboard_view_controller;
pub mod gk_local_player;
mod gk_score;
mod gk_session;

/// Per-process state for the GameKit framework.
#[derive(Default)]
pub struct State {
    pub local_player: gk_local_player::State,
}

// Added to fix _GKErrorDomain crash
pub const CONSTANTS: ConstantExports = &[
    ("_GKErrorDomain", HostConstant::NSString("GKErrorDomain")),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/GameKit.framework/GameKit",
    aliases: &[],
    class_exports: &[
        ad_banner_view::CLASSES,
        fb_session::CLASSES, 
        gk_leaderboard_view_controller::CLASSES,
        gk_local_player::CLASSES,
        gk_score::CLASSES,
        gk_session::CLASSES,
    ],
    constant_exports: &[
        gk_local_player::CONSTANTS, 
        ad_banner_view::CONSTANTS,
        CONSTANTS, // Added our new constants here
    ],
    function_exports: &[],
};
