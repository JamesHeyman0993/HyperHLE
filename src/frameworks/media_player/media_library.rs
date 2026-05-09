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
    
    // Instead of returning nil, we return a valid allocated instance of this class.
    // This stops the crash because the game receives a real pointer.
    let class = env.objc.classes.get("MPMediaLibrary");
    class.alloc(env)
}

@end

};
