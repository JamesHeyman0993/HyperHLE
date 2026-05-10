/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaLibrary`.

use crate::{
    dyld::{ConstantExports, HostConstant},
    objc::{id, nil, objc_classes, ClassExports},
};
use crate::msg;

pub const MPMediaLibraryDidChangeNotification: &str = "MPMediaLibraryDidChangeNotification";

// Additional constants for Gameloft compatibility
pub const MPMediaItemPropertyTitle: &str = "title";
pub const MPMediaItemPropertyArtist: &str = "artist";
pub const MPMediaItemPropertyAlbumTitle: &str = "albumTitle";
pub const MPMediaPlaylistPropertyName: &str = "name";

pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMediaLibraryDidChangeNotification",
        HostConstant::NSString(MPMediaLibraryDidChangeNotification),
    ),
    (
        "_MPMediaItemPropertyTitle",
        HostConstant::NSString(MPMediaItemPropertyTitle),
    ),
    (
        "_MPMediaItemPropertyArtist",
        HostConstant::NSString(MPMediaItemPropertyArtist),
    ),
    (
        "_MPMediaItemPropertyAlbumTitle",
        HostConstant::NSString(MPMediaItemPropertyAlbumTitle),
    ),
    (
        "_MPMediaPlaylistPropertyName",
        HostConstant::NSString(MPMediaPlaylistPropertyName),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaLibrary: NSObject

+ (id)defaultMediaLibrary {
    log!("Gameloft Hack: defaultMediaLibrary (alloc + init)");
    let class_ptr = env.objc.get_known_class("MPMediaLibrary", &mut env.mem);
    let instance: id = msg![env; class_ptr alloc];
    msg![env; instance init]
}

+ (u32)authorizationStatus {
    // Return MPMediaLibraryAuthorizationStatusAuthorized = 3
    3
}

// Gameloft often calls this to see when the library was last updated
- (id)lastModifiedDate {
    // Returning nil is usually safe, or we could return [NSDate date]
    nil
}

// This is a common point of failure. The game asks for all songs/playlists.
- (id)collectionsForEntityKind:(u32)kind {
    log!("Gameloft Hack: collectionsForEntityKind requested (kind: {}), returning nil.", kind);
    nil
}

@end

};
