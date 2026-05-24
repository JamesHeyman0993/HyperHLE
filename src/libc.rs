/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Our implementations of various things that Apple's libSystem would provide.
//!
//! On other platforms these are part of the "libc", so let's call it that.
//!
//! Useful resources:
//!
//! - Apple's [iOS Manual Pages](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/) (contains what would be `man` pages if iOS had a command line)

pub mod arpa;
pub mod asl;
pub mod blocks;
pub mod clocale;
pub mod crypto;
pub mod ctype;
pub mod cxxabi;
pub mod dirent;
pub mod dispatch;
pub mod dlfcn;
pub mod dns_sd;
pub mod errno;
pub mod fnmatch;
mod generic_char;
pub mod glob;
pub mod ifaddrs;
pub mod keymgr;
pub mod libkern;
pub mod mach;
pub mod mach_o;
pub mod math;
pub mod mmap;
pub mod net;
pub mod netdb;
pub mod posix_io;
pub mod pthread;
pub mod sched;
pub mod semaphore;
pub mod setjmp;
pub mod signal;
pub mod ssp;
pub mod stdio;
pub mod stdlib;
pub mod string;
pub mod sys;
pub mod sysctl;
pub mod time;
pub mod unistd;
pub mod wchar;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/usr/lib/libSystem.B.dylib",
    aliases: &["/usr/lib/libSystem.dylib"],
    class_exports: &[],
    constant_exports: &[
        ctype::CONSTANTS,
        dispatch::CONSTANTS,
        stdio::CONSTANTS,
        mach::init::CONSTANTS,
        ssp::CONSTANTS,
    ],
    function_exports: &[
        arpa::inet::FUNCTIONS,
        asl::FUNCTIONS,
        blocks::FUNCTIONS,
        clocale::FUNCTIONS,
        ctype::FUNCTIONS,
        cxxabi::FUNCTIONS,
        crypto::FUNCTIONS,
        dirent::FUNCTIONS,
        dispatch::FUNCTIONS,
        dlfcn::FUNCTIONS,
        dns_sd::FUNCTIONS,
        errno::FUNCTIONS,
        fnmatch::FUNCTIONS,
        glob::FUNCTIONS,
        ifaddrs::FUNCTIONS,
        keymgr::FUNCTIONS,
        libkern::os_atomic::FUNCTIONS,
        mach::arm::task::FUNCTIONS,
        mach::arm::thread_act::FUNCTIONS,
        libkern::task::FUNCTIONS,
        mach::host::FUNCTIONS,
        mach::init::FUNCTIONS,
        mach::mach_port::FUNCTIONS,
        mach::message::FUNCTIONS,
        mach::semaphore::FUNCTIONS,
        mach::thread_info::FUNCTIONS,
        mach::time::FUNCTIONS,
        mach::vm_map::FUNCTIONS,
        mach_o::FUNCTIONS,
        math::FUNCTIONS,
        mmap::FUNCTIONS,
        net::if_::FUNCTIONS,
        netdb::FUNCTIONS,
        posix_io::FUNCTIONS,
        posix_io::stat::FUNCTIONS,
        posix_io::statvfs::FUNCTIONS,
        pthread::cond::FUNCTIONS,
        pthread::key::FUNCTIONS,
        pthread::mutex::FUNCTIONS,
        pthread::once::FUNCTIONS,
        pthread::thread::FUNCTIONS,
        sched::FUNCTIONS,
        semaphore::FUNCTIONS,
        setjmp::FUNCTIONS,
        signal::FUNCTIONS,
        ssp::FUNCTIONS,
        stdio::FUNCTIONS,
        stdio::printf::FUNCTIONS,
        stdlib::FUNCTIONS,
        stdlib::qsort::FUNCTIONS,
        string::FUNCTIONS,
        sys::mount::FUNCTIONS,
        sys::ptrace::FUNCTIONS,
        sys::timeb::FUNCTIONS,
        sys::socket::FUNCTIONS,
        sys::utsname::FUNCTIONS,
        sys::wait::FUNCTIONS,
        sysctl::FUNCTIONS,
        time::FUNCTIONS,
        unistd::FUNCTIONS,
        wchar::FUNCTIONS,
    ],
};

/// Container for state of various child modules
#[derive(Default)]
pub struct State {
    dirent: dirent::State,
    dispatch: dispatch::State,
    keymgr: keymgr::State,
    math: math::State,
    netdb: netdb::State,
    posix_io: posix_io::State,
    pub pthread: pthread::State,
    pub semaphore: semaphore::State,
    pub socket: sys::socket::State,
    stdlib: stdlib::State,
    string: string::State,
    signal: signal::State,
    stdio: stdio::State,
    time: time::State,
    errno: errno::State,
    clocale: clocale::State,
    mach_vm: mach::vm_map::State,
    mmap: mmap::State,
}

// =========================================================================
// HyperHLE App Initialization Overrides (Global Symbol Exports)
// =========================================================================

#[no_mangle]
pub extern "C" fn _ZN5boost14singleton_poolINS_18pool_allocator_tagELj140EN6glotv316event_new_deleteENSt3__15mutexELj128ELj0EE7is_fromEPv(_env: &mut crate::Environment, _ptr: u32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn _ZN5boost14singleton_poolINS_18pool_allocator_tagELj80EN6glotv321event_list_new_deleteENSt3__15mutexELj16ELj0EE7is_fromEPv(_env: &mut crate::Environment, _ptr: u32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn _ZN5boost14singleton_poolINS_18pool_allocator_tagELj208EN6glotv323async_client_new_deleteENSt3__15mutexELj16ELj0EE7is_fromEPv(_env: &mut crate::Environment, _ptr: u32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn _ZN5boost6random23mersenne_twister_engineIjLm32ELm624ELm397ELm31ELj2567483615ELm11ELj4294967295ELm7ELj2636928640ELm15ELj4022730752ELm18ELj1812433253EE4seedEv(_env: &mut crate::Environment) {
    // Intentionally left empty
}

#[no_mangle]
pub extern "C" fn _ZNSt3__16vectorINS_12basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEENS4_IS6_EEE8__appendEm(_env: &mut crate::Environment, _vector_ptr: u32, _count: u32) {
    // Intentionally left empty
}

#[no_mangle]
pub extern "C" fn _ZN5boost5uuids22basic_random_generatorINS_6random23mersenne_twister_engineIjLm32ELm624ELm397ELm31ELj2567483615ELm11ELj4294967295ELm7ELj2636928640ELm15ELj4022730752ELm18ELj1812433253EEEEC2Ev(_env: &mut crate::Environment, this_ptr: u32) -> u32 {
    this_ptr
}

#[no_mangle]
pub extern "C" fn _ZN5boost9gregorian4dateC2ENS0_9greg_yearENS0_10greg_monthENS0_8greg_dayE(_env: &mut crate::Environment, this_ptr: u32, _y: u32, _m: u32, _d: u32) -> u32 {
    this_ptr
}

#[no_mangle]
pub extern "C" fn _ZN5boost9date_time16counted_time_repINS_10posix_time33millisec_posix_time_system_configEEC2ERKNS_9gregorian4dateERKNS2_13time_durationE(_env: &mut crate::Environment, this_ptr: u32, _date_ptr: u32, _duration_ptr: u32) -> u32 {
    this_ptr
}

#[no_mangle]
pub extern "C" fn _ZN3glf7TlsNodeC2IPNS_6ThreadEEEPKT_(_env: &mut crate::Environment, this_ptr: u32, _arg_ptr: u32) -> u32 {
    this_ptr
}

#[no_mangle]
pub extern "C" fn _ZN3glf7TlsNodeC2INS_6Thread9NativeTlsEEEPKT_(_env: &mut crate::Environment, this_ptr: u32, _arg_ptr: u32) -> u32 {
    this_ptr
}

#[no_mangle]
pub extern "C" fn _ZN3glf7TlsNodeC2IPNS_11task_detail5GroupEEEPKT_(_env: &mut crate::Environment, this_ptr: u32, _arg_ptr: u32) -> u32 {
    this_ptr
}
