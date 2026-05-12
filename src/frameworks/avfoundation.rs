/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! The AVFoundation framework.

mod av_audio_player;
pub mod av_audio_session;
pub mod av_capture;

use crate::objc::id;
use crate::dyld::HostConstant; // Added import
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub av_audio_session: av_audio_session::State,
    pub av_capture: av_capture::State,
    pub av_capture_preview_extras: HashMap<id, av_capture::AVCapturePreviewLayerExtra>,
}

pub const CONSTANTS: crate::dyld::ConstantExports = &[
    ("_kCMTimeZero", HostConstant::NSString("kCMTimeZero")),
    ("_AVPlayerItemDidPlayToEndTimeNotification", HostConstant::NSString("AVPlayerItemDidPlayToEndTimeNotification")),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/AVFoundation.framework/AVFoundation",
    aliases: &[],
    class_exports: &[
        av_audio_player::CLASSES,
        av_audio_session::CLASSES,
        av_capture::CLASSES,
    ],
    constant_exports: &[
        av_audio_session::CONSTANTS, 
        av_capture::CONSTANTS,
        CONSTANTS, // Added our new constants here
    ],
    function_exports: &[],
};
