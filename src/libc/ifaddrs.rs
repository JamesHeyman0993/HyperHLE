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
    // 1. Allocate space for the string "en0\0" (4 bytes)
    // We use a dummy byte to start the allocation
    let name_bytes = b"en0\0";
    let fake_name_ptr = env.mem.alloc_and_write(name_bytes[0]); // Allocates 1 byte and writes 'e'
    
    // Write the rest of the bytes manually to the memory location
    // (Assuming your Mem has a basic write or write_bytes method)
    for (i, &byte) in name_bytes.iter().enumerate() {
        env.mem.write(fake_name_ptr.offset(i as isize), byte);
    }
    
    // 2. Set flags: IFF_UP (0x1) | IFF_RUNNING (0x40) | IFF_BROADCAST (0x2)
    let active_flags: u32 = 0x1 | 0x40 | 0x02;

    // 3. Allocate and write the fake ifaddrs struct
    let fake_if = env.mem.alloc_and_write(ifaddrs {
        ifa_next: MutPtr::null(),
        // Use .cast() to turn Ptr<u8> into the expected ConstPtr<u8>
        ifa_name: fake_name_ptr.cast_const(),
        ifa_flags: active_flags,
        ifa_addr: 0,
        ifa_netmask: 0,
        ifa_broadaddr: 0,
        ifa_data: 0,
    });

    // 4. Point the game to our fake interface
    if !ifap.is_null() {
        env.mem.write(ifap, fake_if);
    }

    log!("getifaddrs(): Reporting ACTIVE fake en0 interface to Archetype.");
    0 
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
