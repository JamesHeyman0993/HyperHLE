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

/// Notification name used by Gameloft games to listen for library changes.
pub const MPMediaLibraryDidChangeNotification: &str = "MPMediaLibraryDidChangeNotification";

pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMediaLibraryDidChangeNotification",
        HostConstant::NSString(MPMediaLibraryDidChangeNotification),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaLibrary: NSObject

+ (id)defaultMediaLibrary {
    log!("Applying Spider-Man/Gameloft hack: Sending alloc AND init to MPMediaLibrary.");

    let class_ptr = env.objc.link_class("MPMediaLibrary", false, &mut env.mem);

    // 1. Allocate the object
    let instance = msg![env; class_ptr alloc];
    
    // 2. Initialize the object so it is not a null/garbage pointer
    msg![env; instance init]
}

+ (u32)authorizationStatus {
    log!("Gameloft Hack: reporting MediaLibrary authorizationStatus as Authorized (3)");
    // 3 corresponds to MPMediaLibraryAuthorizationStatusAuthorized
    3
}

@end

};
