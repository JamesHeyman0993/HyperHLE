/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMusicPlayerController` etc.

use crate::{
    dyld::{ConstantExports, HostConstant},
    objc::{id, nil, objc_classes, ClassExports},
};
use crate::msg; // <--- Add this

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
    log!("Gameloft Hack: returning dummy iPodMusicPlayer");
    let class_ptr = env.objc.link_class("MPMusicPlayerController", false, &mut env.mem);
    msg![env; class_ptr alloc]
}

+ (id)applicationMusicPlayer {
    log!("Gameloft Hack: returning dummy applicationMusicPlayer");
    let class_ptr = env.objc.link_class("MPMusicPlayerController", false, &mut env.mem);
    msg![env; class_ptr alloc]
}

@end

};
