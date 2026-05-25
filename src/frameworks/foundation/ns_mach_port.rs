/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSMachPort` implementation

use crate::objc::{id, msg_super, objc_classes, ClassExports, HostObject, NSZonePtr};
use crate::Environment;

pub struct NSMachPortHostObject {}
impl HostObject for NSMachPortHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);

@implementation NSMachPort : NSPort

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSMachPortHostObject {});
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    let this: id = msg_super![env; this init];
    log!("HyperHLE: NSMachPort allocated and stubbed cleanly.");
    this
}

@end
};
