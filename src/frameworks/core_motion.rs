/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Motion framework.
use crate::{msg, msg_class}; // Add this line
use crate::dyld::HostDylib;
use crate::objc::{id, nil, objc_classes, ClassExports};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreMotion.framework/CoreMotion",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// --- New Stub Classes ---

@implementation CMDeviceMotion: NSObject
// Providing a valid instance for CMMotionManager to return
@end

@implementation CMAccelerometerData: NSObject
// Providing a valid instance for CMMotionManager to return
@end

@implementation CMGyroData: NSObject
// Providing a valid instance for CMMotionManager to return
@end

// --- Existing Implementation ---

@implementation CMMotionManager: NSObject

- (bool)isGyroAvailable {
    true
}
- (bool)isDeviceMotionAvailable {
    true
}
- (bool)isAccelerometerAvailable {
    true
}

- (())setAccelerometerUpdateInterval:(f64)interval {
    log_dbg!("[(CMMotionManager *){:?} setAccelerometerUpdateInterval:{}]", this, interval);
}

- (())startAccelerometerUpdates {
    log_dbg!("[(CMMotionManager *){:?} startAccelerometerUpdates]", this);
}

- (())setGyroUpdateInterval:(f64)interval {
    log_dbg!("[(CMMotionManager *){:?} setGyroUpdateInterval:{}]", this, interval);
}

- (())startGyroUpdates {
    log_dbg!("[(CMMotionManager *){:?} startGyroUpdates]", this);
}

- (())setDeviceMotionUpdateInterval:(f64)interval {
    log_dbg!("[(CMMotionManager *){:?} setDeviceMotionUpdateInterval:{}]", this, interval);
}

- (())startDeviceMotionUpdates {
    log_dbg!("[(CMMotionManager *){:?} startDeviceMotionUpdates]", this);
}

- (bool)isDeviceMotionActive {
    true
}

- (bool)isAccelerometerActive {
    true
}

- (bool)isGyroActive {
    true
}

// --- The Crash Fixes ---

- (id)deviceMotion {
    log!("HACK: [(CMMotionManager *){:?} deviceMotion] -> lock-safe stub", this);
    
    // 1. Look up the class pointer from the registry
    let cls = env.objc.class_get("CMDeviceMotion").expect("CMDeviceMotion class not found");
    
    // 2. Create the instance. In many touchHLE versions, this is called 'new_instance_for_class'
    // or we use the host object allocation directly. Let's try the safest path:
    let stub = env.objc.borrow_mut::<crate::objc::ObjC>(this).new_instance_for_class(cls);
    
    crate::objc::autorelease(env, stub);
    stub
}

- (id)accelerometerData {
    log!("HACK: [(CMMotionManager *){:?} accelerometerData] -> lock-safe stub", this);
    let cls = env.objc.class_get("CMAccelerometerData").expect("CMAccelerometerData class not found");
    let stub = env.objc.borrow_mut::<crate::objc::ObjC>(this).new_instance_for_class(cls);
    crate::objc::autorelease(env, stub);
    stub
}

- (id)gyroData {
    log!("HACK: [(CMMotionManager *){:?} gyroData] -> lock-safe stub", this);
    let cls = env.objc.class_get("CMGyroData").expect("CMGyroData class not found");
    let stub = env.objc.borrow_mut::<crate::objc::ObjC>(this).new_instance_for_class(cls);
    crate::objc::autorelease(env, stub);
    stub
}
                
@end

};
