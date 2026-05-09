/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaItemCollection`.

use crate::objc::{id, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaItemCollection: MPMediaEntity

- (id)initWithItems:(id)items {
    log!("Gameloft Hack: Initializing dummy MediaItemCollection with provided items.");
    this
}

- (u32)count { // <--- Changed uint32_t to u32 here
    0
}

- (id)items {
    crate::objc::nil
}

@end

};
