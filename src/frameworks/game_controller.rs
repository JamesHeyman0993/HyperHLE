/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::dyld::{ConstantExports, HostConstant, HostDylib};
use crate::objc::{objc_classes, ClassExports, id, SEL, nil};

pub const CONSTANTS: ConstantExports = &[
    (
        "_GCControllerDidConnectNotification",
        HostConstant::NSString("GCControllerDidConnectNotification"),
    ),
    (
        "_GCControllerDidDisconnectNotification",
        HostConstant::NSString("GCControllerDidDisconnectNotification"),
    ),
];

// Define a minimal working class implementation for GCController
pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation GCController : NSObject

    // Class method fallback handler
    + (bool)respondsToSelector:(SEL)selector {
        log!("HyperHLE: GCController class respondsToSelector check bypass -> returning true");
        true
    }

    // Instance method fallback handler
    - (bool)respondsToSelector:(SEL)selector {
        log!("HyperHLE: GCController instance respondsToSelector check bypass -> returning true");
        true
    }

    // Provide a standard array initializer in case it checks connected controllers
    + (id)controllers {
        log!("HyperHLE: [GCController controllers] called -> returning empty array representation");
        // Fallback to nil or an empty array instance if your foundation layer supports it
        nil 
    }
    @end
};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/GameController.framework/GameController",
    aliases: &[],
    class_exports: &[CLASSES], // <--- Added the exported class structure here
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};
