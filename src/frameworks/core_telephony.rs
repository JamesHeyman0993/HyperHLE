/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CoreTelephony framework stubs.

use crate::objc::{objc_classes, ClassExports, id};

// Satisfy the macro's need for a 'void' type
type void = ();

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation CTTelephonyNetworkInfo : NSObject
    - (id)init { this }
    - (id)subscriberCellularProvider { crate::objc::nil }
    - (id)currentRadioAccessTechnology { crate::objc::nil }
    @end

    @implementation CTCarrier : NSObject
    - (id)carrierName { crate::objc::nil }
    - (id)mobileCountryCode { crate::objc::nil }
    - (id)mobileNetworkCode { crate::objc::nil }
    - (id)isoCountryCode { crate::objc::nil }
    - (bool)allowsVOIP { false }
    @end
};

pub const DYLIB: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CoreTelephony.framework/CoreTelephony",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};
