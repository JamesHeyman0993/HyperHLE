/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
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
    pub ifa_addr: u32, 
    pub ifa_netmask: u32,
    pub ifa_broadaddr: u32,
    pub ifa_data: u32,
}
unsafe impl SafeRead for ifaddrs {}

// ---------------------------------------------------------------------------
// getifaddrs / freeifaddrs
// ---------------------------------------------------------------------------

/// `int getifaddrs(struct ifaddrs **ifap)`
fn getifaddrs(env: &mut Environment, ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    // 1. Prepare a simple name pointer. 
    // We'll just allocate a single byte 'e' to represent the name for now 
    // to see if the game just wants a non-null pointer.
    let fake_name_ptr = env.mem.alloc_and_write(b'e');
    
    // 2. Flags: UP | RUNNING | BROADCAST
    let active_flags: u32 = 0x1 | 0x40 | 0x02;

    // 3. Create the struct
    let fake_if = env.mem.alloc_and_write(ifaddrs {
        ifa_next: MutPtr::null(),
        ifa_name: fake_name_ptr.cast_const(),
        ifa_flags: active_flags,
        ifa_addr: 0,
        ifa_netmask: 0,
        ifa_broadaddr: 0,
        ifa_data: 0,
    });

    // 4. Update the pointer provided by the game
    if !ifap.is_null() {
        env.mem.write(ifap, fake_if);
        log!("getifaddrs(): Provided fake interface en0 at {:?}", fake_if);
        0 
    } else {
        set_errno(env, EINVAL);
        -1
    }
}

/// `void freeifaddrs(struct ifaddrs *ifa)`
fn freeifaddrs(_env: &mut Environment, _ifa: MutPtr<ifaddrs>) {
    // No-op. Since we returned an empty list (NULL), there is nothing to free.
}

// ---------------------------------------------------------------------------
// net/if.h – interface index / name mapping
// ---------------------------------------------------------------------------

const IF_NAMESIZE: usize = 16;

fn if_nametoindex(env: &mut Environment, ifname: ConstPtr<u8>) -> u32 {
    let name = env.mem.cstr_at_utf8(ifname).unwrap_or("<invalid>");
    log!("if_nametoindex(\"{}\") – returning 0", name);
    set_errno(env, ENXIO);
    0
}

fn if_indextoname(env: &mut Environment, ifindex: u32, ifname: MutPtr<u8>) -> MutPtr<u8> {
    log!("if_indextoname({}) – returning NULL", ifindex);
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
    // Return NULL but don't set an error; let the app think the list is just empty.
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
