/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! The AVFoundation framework.

mod av_audio_player;
pub mod av_audio_session;
pub mod av_capture;

use crate::objc::{id, msg_class, msg, objc_classes, ClassExports, NSZonePtr, nil};
use crate::dyld::HostConstant;
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

// --- MOCK FOR AVURLAsset ---
pub const MOCK_CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation AVURLAsset: NSObject

        + (id)allocWithZone:(NSZonePtr)_zone {
        // Create an empty dummy structure to act as the host object layout wrapper
        struct DummyAsset;
        impl crate::objc::HostObject for DummyAsset {}

        env.objc.alloc_object(this, Box::new(DummyAsset), &mut env.mem)
        }
    
    + (id)URLAssetWithURL:(id)url options:(id)options {
        log!("HACK: Mocking [AVURLAsset URLAssetWithURL:] to prevent null path crash.");
        
        let asset: id = msg_class![env; AVURLAsset alloc];
        let asset: id = msg![env; asset init];
        asset
    }

    @end
};

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/AVFoundation.framework/AVFoundation",
    aliases: &[],
    class_exports: &[
        av_audio_player::CLASSES,
        av_audio_session::CLASSES,
        av_capture::CLASSES,
        MOCK_CLASSES, // Added our new mock class here
    ],
    constant_exports: &[
        av_audio_session::CONSTANTS, 
        av_capture::CONSTANTS,
        CONSTANTS,
    ],
    function_exports: &[],
};
