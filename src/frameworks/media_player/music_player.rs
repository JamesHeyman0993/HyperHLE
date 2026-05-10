/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMusicPlayerController` etc.

use crate::{
    dyld::{ConstantExports, HostConstant},
    objc::{id, nil, objc_classes, ClassExports},
    frameworks::foundation::NSInteger, // Import from foundation instead
};

pub const MPMusicPlayerControllerNowPlayingItemDidChangeNotification: &str =
    "MPMusicPlayerControllerNowPlayingItemDidChangeNotification";
pub const MPMusicPlayerControllerPlaybackStateDidChangeNotification: &str =
    "MPMusicPlayerControllerPlaybackStateDidChangeNotification";
pub const MPMediaItemPropertyPersistentID: &str = "persistentID";

/// `NSNotificationName` values.
pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMusicPlayerControllerNowPlayingItemDidChangeNotification",
        HostConstant::NSString(MPMusicPlayerControllerNowPlayingItemDidChangeNotification),
    ),
    (
        "_MPMusicPlayerControllerPlaybackStateDidChangeNotification",
        HostConstant::NSString(MPMusicPlayerControllerPlaybackStateDidChangeNotification),
    ),
    (
        "_MPMediaItemPropertyPersistentID",
        HostConstant::NSString(MPMediaItemPropertyPersistentID),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMusicPlayerController: NSObject

+ (id)iPodMusicPlayer {
    log!("Gameloft Hack: returning dummy iPodMusicPlayer (alloc + init)");
    let class_ptr = env.objc.get_known_class("MPMusicPlayerController", &mut env.mem);
    
    // Allocate and Initialize
    let instance: id = crate::objc::msg![env; class_ptr alloc];
    crate::objc::msg![env; instance init]
}

+ (id)applicationMusicPlayer {
    log!("Gameloft Hack: returning dummy applicationMusicPlayer (alloc + init)");
    let class_ptr = env.objc.get_known_class("MPMusicPlayerController", &mut env.mem);
    
    let instance: id = crate::objc::msg![env; class_ptr alloc];
    crate::objc::msg![env; instance init]
}

// --- NEW STUBS TO PREVENT CRASH ---

- (())beginGeneratingPlaybackNotifications {
    log!("Stubbed beginGeneratingPlaybackNotifications");
}

- (())endGeneratingPlaybackNotifications {
    log!("Stubbed endGeneratingPlaybackNotifications");
}

- (NSInteger)playbackState {
    // Return 0 (MPMusicPlaybackStateStopped)
    0
}

- (())setQueueWithQuery:(id)_query {
    log!("Stubbed setQueueWithQuery:");
}

- (())play {
    log!("Stubbed MPMusicPlayerController play");
}

- (())stop {
    log!("Stubbed MPMusicPlayerController stop");
}

@end
    
};
