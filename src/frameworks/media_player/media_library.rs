/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaLibrary`.

use crate::objc::{id, nil, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaLibrary: NSObject

+ (id)defaultMediaLibrary {
    log!("Applying Spider-Man/Gameloft hack: returning dummy MPMediaLibrary to prevent NULL-PAGE READ.");
    
    // 1. Get the class. link_class ensures it exists and returns a Class handle.
    // false = not a metaclass.
    let class = env.objc.link_class("MPMediaLibrary", false, &mut env.mem);
    
    // 2. Allocate the object
    class.alloc(env)
}
    
@end

};
