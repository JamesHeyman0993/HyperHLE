/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaQuery`.

use crate::objc::{id, nil, objc_classes, ClassExports};
use crate::msg;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaQuery: NSObject

+ (id)playlistsQuery {
    log!("Gameloft Hack: returning dummy playlistsQuery (alloc + init)");
    let class_ptr = env.objc.link_class("MPMediaQuery", false, &mut env.mem);
    
    // 1. Allocate
    let instance = msg![env; class_ptr alloc];
    
    // 2. Initialize
    msg![env; instance init]
}

+ (id)songsQuery {
    log!("Gameloft Hack: returning dummy songsQuery (alloc + init)");
    let class_ptr = env.objc.link_class("MPMediaQuery", false, &mut env.mem);
    
    // 1. Allocate
    let instance = msg![env; class_ptr alloc];
    
    // 2. Initialize
    msg![env; instance init]
}

@end

};
