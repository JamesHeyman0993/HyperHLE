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
    
    // Use the public method to get the class instead of accessing the private field
    let class = env.objc.get_class("MPMediaLibrary");
    class.alloc(env)
}
    
@end

};
