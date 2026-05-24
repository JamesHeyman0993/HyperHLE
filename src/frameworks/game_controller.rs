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

            // ==========================================
    // CLASS METHODS (+) MUST GO FIRST
    // ==========================================
    + (bool)respondsToSelector:(SEL)selector {
        log!("HyperHLE: GCController class respondsToSelector check bypass -> returning true");
        true
    }

    + (id)controllers {
        log!("HyperHLE: [GCController controllers] called -> fetching an empty NSArray instance");
        // Dynamically locate the NSArray class template inside the runtime environment
        let array_class = env.objc.get_known_class("NSArray", &mut env.mem);
        
        // Execute the native [NSArray array] message allocation
        let empty_array: id = crate::objc::msg![env; array_class array];
        empty_array
    }
    
    // ==========================================
    // INSTANCE METHODS (-) MUST GO SECOND
    // ==========================================
    - (bool)respondsToSelector:(SEL)selector {
        log!("HyperHLE: GCController instance respondsToSelector check bypass -> returning true");
        true
    }
    @end
};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/GameController.framework/GameController",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};
