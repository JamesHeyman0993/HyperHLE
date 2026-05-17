/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ifaddrs.h` and `net/if.h` (interface addresses and interface naming)

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::{set_errno, EINVAL, ENXIO};
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::Environment;

// Mirrors the POSIX `struct ifaddrs` layout as seen by 32-bit ARM guests.
#[allow(non_camel_case_types)]
#[repr(C, packed)]
pub struct ifaddrs {
    pub ifa_next: MutPtr<ifaddrs>,
    pub ifa_name: ConstPtr<u8>,
    pub ifa_flags: u32,
    pub ifa_addr: u32, // Pointer to struct sockaddr
    pub ifa_netmask: u32, // Pointer to struct sockaddr
    pub ifa_broadaddr: u32, // Pointer to struct sockaddr
    pub ifa_data: u32,
}
unsafe impl SafeRead for ifaddrs {}

// Minimal BSD sockaddr layout for 32-bit iOS/ARM guests
#[allow(non_camel_case_types)]
#[repr(C, packed)]
struct sockaddr_in {
    sin_len: u8,
    sin_family: u8,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}
unsafe impl SafeRead for sockaddr_in {}

// ---------------------------------------------------------------------------
// getifaddrs / freeifaddrs
// ---------------------------------------------------------------------------

/// `int getifaddrs(struct ifaddrs **ifap)`
fn getifaddrs(env: &mut Environment, ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    if ifap.is_null() {
        set_errno(env, EINVAL);
        return -1;
    }

    // Force the emulator to report a standard network subsystem error (-1).
    // This tells the guest game that there are absolutely no network hardware
    // configurations available, bypassing ad overlay loads.
    log!("getifaddrs(): Faking completely offline state to bypass ad overlays.");
    set_errno(env, ENXIO);
    -1
}

/// `void freeifaddrs(struct ifaddrs *ifa)`
fn freeifaddrs(_env: &mut Environment, _ifa: MutPtr<ifaddrs>) {
    // No-op
}

// ---------------------------------------------------------------------------
// net/if.h – interface index / name mapping
// ---------------------------------------------------------------------------

fn if_nametoindex(env: &mut Environment, ifname: ConstPtr<u8>) -> u32 {
    let name = env.mem.cstr_at_utf8(ifname).unwrap_or("<invalid>");
    log!("if_nametoindex(\"{}\") – returning 0 (No device found)", name);
    0
}

fn if_indextoname(env: &mut Environment, ifindex: u32, _ifname: MutPtr<u8>) -> MutPtr<u8> {
    // Return NULL to keep network queries completely isolated and empty
    log!("if_indextoname({}) – offline mode active, returning NULL", ifindex);
    set_errno(env, ENXIO);
    MutPtr::null()
}

#[allow(non_camel_case_types)]
#[repr(C, packed)]
pub struct if_nameindex {
    pub if_index: u32,
    pub if_name: ConstPtr<u8>,
}
unsafe impl SafeRead for if_nameindex {}

fn if_nameindex(_env: &mut Environment) -> MutPtr<if_nameindex> {
    MutPtr::null()
}

fn if_freenameindex(_env: &mut Environment, _ptr: MutPtr<if_nameindex>) {
    // No-op
}

// ---------------------------------------------------------------------------
// Export table
// ---------------------------------------------------------------------------

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getifaddrs(_)),
    export_c_func!(freeifaddrs(_)),
    export_c_func!(if_nametoindex(_)),
    export_c_func!(if_indextoname(_, _)),
    export_c_func!(if_nameindex()),
    export_c_func!(if_freenameindex(_)),
];
