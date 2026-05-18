/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Foundation framework.

pub mod cf_allocator;
pub mod cf_array;
pub mod cf_bundle;
pub mod cf_data;
pub mod cf_dictionary;
pub mod cf_host;
pub mod cf_locale;
pub mod cf_number;
pub mod cf_preferences;
pub mod cf_run_loop;
pub mod cf_run_loop_timer;
pub mod cf_socket;
pub mod cf_stream;
pub mod cf_string;
pub mod cf_type;
pub mod cf_url;
pub mod cf_uuid;
pub mod time;

use crate::dyld::{ConstantExports, HostConstant}; // Added for constants support

pub const CONSTANTS: ConstantExports = &[
    ("_kCFNull", HostConstant::NullPtr),
    ("_kCFNumberNaN", HostConstant::NullPtr),
    ("_kCFCoreFoundationVersionNumber", HostConstant::NullPtr),
    // FIXED: Added missing infinity constants requested by Gumball layout engine
    ("_kCFNumberNegativeInfinity", HostConstant::NullPtr),
    ("_kCFNumberPositiveInfinity", HostConstant::NullPtr),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation",
    aliases: &[],
    class_exports: &[
        cf_run_loop_timer::CLASSES,
        cf_host::CLASSES,
        cf_stream::CLASSES,
        cf_uuid::CLASSES,
    ],
    constant_exports: &[
        cf_allocator::CONSTANTS,
        cf_bundle::CONSTANTS,
        cf_dictionary::CONSTANTS,
        cf_locale::CONSTANTS,
        cf_number::CONSTANTS,
        cf_preferences::CONSTANTS,
        cf_run_loop::CONSTANTS,
        cf_stream::CONSTANTS,
        cf_url::CONSTANTS,
        time::CONSTANTS,
        CONSTANTS, // Added our new Ghost Toasters fixes here
    ],
    function_exports: &[
        FUNCTIONS,
        cf_allocator::FUNCTIONS,
        cf_array::FUNCTIONS,
        cf_dictionary::FUNCTIONS,
        cf_bundle::FUNCTIONS,
        cf_socket::FUNCTIONS,
        cf_data::FUNCTIONS,
        cf_host::FUNCTIONS,
        cf_locale::FUNCTIONS,
        cf_number::FUNCTIONS,
        cf_preferences::FUNCTIONS,
        cf_run_loop::FUNCTIONS,
        cf_run_loop_timer::FUNCTIONS,
        cf_string::FUNCTIONS,
        cf_stream::FUNCTIONS,
        cf_type::FUNCTIONS,
        cf_url::FUNCTIONS,
        cf_uuid::FUNCTIONS,
        time::FUNCTIONS,
    ],
};

pub use cf_type::{CFRelease, CFRetain, CFTypeRef};

pub type CFHashCode = u32;
pub type CFIndex = i32;
pub type CFOptionFlags = u32;
pub type CFComparisonResult = CFIndex;

use crate::abi::GuestArg;
use crate::dyld::FunctionExports;
use crate::environment::Environment;
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::mem::SafeRead;
use crate::objc::id;
use crate::{export_c_func, impl_GuestRet_for_large_struct, msg};

pub const kCFNotFound: CFIndex = -1;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

unsafe impl SafeRead for CFRange {}
impl_GuestRet_for_large_struct!(CFRange);
impl GuestArg for CFRange {
    const REG_COUNT: usize = 2;

    fn from_regs(regs: &[u32]) -> Self {
        CFRange {
            location: GuestArg::from_regs(&regs[0..1]),
            length: GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.location.to_regs(&mut regs[0..1]);
        self.length.to_regs(&mut regs[1..2]);
    }
}

fn CFShow(env: &mut Environment, obj: CFTypeRef) {
    let description: id = msg![env; obj description];
    log!("{}", to_rust_string(env, description));
}

const FUNCTIONS: FunctionExports = &[export_c_func!(CFShow(_))];
