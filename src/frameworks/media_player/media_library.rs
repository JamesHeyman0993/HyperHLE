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
    log!("Gameloft Hack: defaultMediaLibrary (alloc + init)");
    let class_ptr = env.objc.link_class("MPMediaLibrary", false, &mut env.mem);
    let instance = msg![env; class_ptr alloc];
    msg![env; instance init]
}

+ (u32)authorizationStatus {
    // Return Authorized
    3
}

// Added these to prevent the game from getting null when it asks for data
- (id)lastModifiedDate {
    log!("Gameloft Hack: ignoring lastModifiedDate");
    nil
}

// Some games check this to see if the library is "ready"
- (bool)isGeniusAvailable {
    false
}

@end

};
