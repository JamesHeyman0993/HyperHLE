/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SystemConfiguration framework.

mod sc_network_reachability;

use crate::dyld::{ConstantExports, HostConstant};

// HyperHLE Patch: Maps the external Wi-Fi initialization key requested by Gameloft titles
pub const CONSTANTS: ConstantExports = &[
    ("_kCNNetworkInfoKeySSID", HostConstant::NullPtr),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/SystemConfiguration.framework/SystemConfiguration",
    aliases: &[],
    class_exports: &[sc_network_reachability::CLASSES],
    constant_exports: &[CONSTANTS], // Registered our new constants table here
    function_exports: &[sc_network_reachability::FUNCTIONS],
};
