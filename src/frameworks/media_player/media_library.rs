/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaLibrary`.

use crate::objc::{autorelease, id, msg, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaLibrary: NSObject

+ (id)defaultMediaLibrary {
    log!("Intercepted [MPMediaLibrary defaultMediaLibrary]: Creating a fake media library instance.");
    
    // Allocate a fake instance of the MPMediaLibrary class so the game gets a valid pointer
    let fake_library: id = msg![env; this alloc];
    let fake_library: id = msg![env; fake_library init];
    
    // Autorelease it to handle memory management correctly
    autorelease(env, fake_library)
}

@end

};
