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

const AF_INET: u8 = 2;

// ---------------------------------------------------------------------------
// getifaddrs / freeifaddrs
// ---------------------------------------------------------------------------

/// `int getifaddrs(struct ifaddrs **ifap)`
fn getifaddrs(env: &mut Environment, ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    if ifap.is_null() {
        set_errno(env, EINVAL);
        return -1;
    }

    // 1. Properly allocate space for "en0\0" by allocating individual u8 bytes
    // to bypass the lack of [u8; 4] trait implementations.
    let fake_name_ptr = env.mem.alloc_and_write(b'e');
    let _n = env.mem.alloc_and_write(b'n');
    let _0 = env.mem.alloc_and_write(b'0');
    let _null = env.mem.alloc_and_write(0u8);
    
    // 2. Flags: UP | RUNNING | BROADCAST | LOOPBACK
    let active_flags: u32 = 0x1 | 0x2 | 0x4 | 0x40;

    // 3. Create a mock sockaddr structure for an IP address (127.0.0.1)
    let fake_addr = env.mem.alloc_and_write(sockaddr_in {
        sin_len: std::mem::size_of::<sockaddr_in>() as u8,
        sin_family: AF_INET,
        sin_port: 0,
        sin_addr: 0x0100007F, // 127.0.0.1 in network byte order
        sin_zero: [0; 8],
    });

    let fake_netmask = env.mem.alloc_and_write(sockaddr_in {
        sin_len: std::mem::size_of::<sockaddr_in>() as u8,
        sin_family: AF_INET,
        sin_port: 0,
        sin_addr: 0x00FFFFFF, // 255.255.255.0
        sin_zero: [0; 8],
    });

    // 4. Create the final ifaddrs struct linking the mock properties
    let fake_if = env.mem.alloc_and_write(ifaddrs {
        ifa_next: MutPtr::null(),
        ifa_name: fake_name_ptr.cast_const(),
        ifa_flags: active_flags,
        ifa_addr: fake_addr.to_bits(),
        ifa_netmask: fake_netmask.to_bits(),
        ifa_broadaddr: MutPtr::<u8>::null().to_bits(),
        ifa_data: 0,
    });

    // 5. Update the pointer provided by the game
    env.mem.write(ifap, fake_if);
    log!("getifaddrs(): Provided stable fake interface en0 at {:?}", fake_if);
    0 
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
    log!("if_nametoindex(\"{}\") – returning 1 for en0", name);
    1
}

fn if_indextoname(env: &mut Environment, ifindex: u32, ifname: MutPtr<u8>) -> MutPtr<u8> {
    if ifindex == 1 && !ifname.is_null() {
        log!("if_indextoname({}) – writing 'en0'", ifindex);
        
        // Write the string sequentially to the target memory buffer pointer
        env.mem.write(ifname, b'e');
        env.mem.write(ifname.offset(1), b'n');
        env.mem.write(ifname.offset(2), b'0');
        env.mem.write(ifname.offset(3), 0u8);
        ifname
    } else {
        log!("if_indextoname({}) – returning NULL", ifindex);
        set_errno(env, ENXIO);
        MutPtr::null()
    }
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
