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
/// CMTime is structurally represented on 32-bit iOS/ARMv7 architectures as:
/// - CMTimeValue (i64, takes R0 and R1 registers)
/// - CMTimeScale (i32, takes R2 register)
/// - CMTimeFlags (u32, takes R3 register)
/// - CMTimeEpoch (i64, pushed to stack)
///
/// For simple engine initializations, filling out the primary value and scale registers 
/// prevents Marmalade loader threads from looping on undefined behavior.
fn CMTimeMake(env: &mut Environment, value: i64, timescale: i32) {
    log!("Stub: CMTimeMake(value: {}, timescale: {}) called.", value, timescale);
    
    // We modify the guest registers directly via CPU state to safely pass a valid 64-bit split CMTimeValue layout back to the caller
    let val_bytes = value.to_ne_bytes();
    let r0 = u32::from_ne_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]);
    let r1 = u32::from_ne_bytes([val_bytes[4], val_bytes[5], val_bytes[6], val_bytes[7]]);
    
    env.cpu.set_r(0, r0);
    env.cpu.set_r(1, r1);
    env.cpu.set_r(2, timescale as u32);
    env.cpu.set_r(3, 1); // CMTimeFlags: kCMTimeFlags_Valid = 1
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
