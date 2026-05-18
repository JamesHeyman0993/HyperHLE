/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! StoreKit

mod sk_payment_queue;
mod sk_product;

use crate::dyld::{ConstantExports, HostConstant};

// FIXED: Initialized constant configuration map block
pub const CONSTANTS: ConstantExports = &[
    (
        "_SKStoreProductParameterITunesItemIdentifier",
        HostConstant::NSString("SKStoreProductParameterITunesItemIdentifier"),
    ),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/StoreKit.framework/StoreKit",
    aliases: &[],
    class_exports: &[sk_payment_queue::CLASSES, sk_product::CLASSES],
    // FIXED: Mounted the array reference slice to expose the parameters safely
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};

#[derive(Default)]
pub struct State {
    pub payment_queue: sk_payment_queue::State,
}
