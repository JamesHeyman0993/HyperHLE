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
    // Return 'this' to show the object was successfully initialized.
    this
}

- (uint32_t)count {
    // Return 0 so the game thinks the playlist/library is empty.
    0
}

- (id)items {
    // If the game asks for the actual list of songs, we return nil (0).
    // Since count is 0, the game shouldn't try to read this list.
    crate::objc::nil
}

@end

};
