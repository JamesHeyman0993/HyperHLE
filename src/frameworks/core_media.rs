/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stub for `CoreMedia.framework/CoreMedia`.
//!
//! On iOS, CoreMedia provides time-related types (`CMTime`, `CMTimeRange`),
//! sample buffer plumbing (`CMSampleBufferRef`), and format descriptions used
//! mostly by AVFoundation. Apps that link against CoreMedia (directly or
//! transitively, e.g. via AVFoundation cutscene playback) put the path
//! `/System/Library/Frameworks/CoreMedia.framework/CoreMedia` in their Mach-O
//! load commands.

use crate::dyld::{export_c_func, FunctionExports, HostConstant};
use crate::Environment;

/// CoreMedia CMTime specification function stub.
/// 
/// Instead of manually breaking down registers, returning a u64 allows touchHLE's 
/// internal framework macro runner to automatically split the 64-bit integer values 
/// across guest registers R0 and R1 to fulfill the ARM EABI calling convention.
fn CMTimeMake(_env: &mut Environment, value: i64, _timescale: i32) -> u64 {
    log!("Stub: CMTimeMake(value: {}, timescale: {}) called.", value, _timescale);
    
    // Cast the i64 timeline parameter into a clean u64 structure payload
    value as u64
}

// Populated missing symbol mapping table for structural time constraints
pub const CONSTANTS: crate::dyld::ConstantExports = &[
    ("_kCMTimeInvalid", HostConstant::NSString("kCMTimeInvalid")),
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CMTimeMake(_, _, _)),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreMedia.framework/CoreMedia",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};
