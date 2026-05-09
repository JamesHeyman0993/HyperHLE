/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaLibrary`.

use crate::objc::{id, nil, objc_classes, ClassExports};
use crate::msg; // <--- Add this line to fix the "cannot find macro" error

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaLibrary: NSObject

+ (id)defaultMediaLibrary {
    log!("Applying Spider-Man/Gameloft hack: Sending alloc AND init to MPMediaLibrary.");

    let class_ptr = env.objc.link_class("MPMediaLibrary", false, &mut env.mem);

    // 1. Allocate the object
    let instance = msg![env; class_ptr alloc];
    
    // 2. Initialize the object (This is the missing step!)
    msg![env; instance init]
}
    
@end

};
