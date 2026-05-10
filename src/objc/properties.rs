/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C properties.

use super::{id, msg, nil, release, retain, Class, ClassHostObject, ObjC, SEL};
use crate::mem::{
    guest_size_of, ConstPtr, ConstVoidPtr, GuestISize, GuestUSize, Mem, MutPtr, MutVoidPtr, Ptr,
    SafeRead,
};
use crate::Environment;

#[repr(C, packed)]
pub(super) struct ivar_list_t {
    entsize: GuestUSize,
    count: GuestUSize,
}
unsafe impl SafeRead for ivar_list_t {}

#[repr(C, packed)]
struct ivar_t {
    offset: ConstPtr<GuestUSize>,
    name: ConstPtr<u8>,
    type_: ConstPtr<u8>,
    alignment: u32,
    size: u32,
}
unsafe impl SafeRead for ivar_t {}

impl ClassHostObject {
    pub(super) fn add_ivars_from_bin(&mut self, ivar_list_ptr: ConstPtr<ivar_list_t>, mem: &Mem) {
        let ivar_list_t { entsize, count } = mem.read(ivar_list_ptr);
        assert!(entsize >= guest_size_of::<ivar_t>());

        let ivars_base_ptr: ConstPtr<ivar_t> = (ivar_list_ptr + 1).cast();

        for i in 0..count {
            let ivar_ptr: ConstPtr<ivar_t> = Ptr::from_bits(ivars_base_ptr.to_bits() + i * entsize);

            let ivar_t {
                offset,
                name,
                alignment,
                ..
            } = mem.read(ivar_ptr);

            let name_string = mem.cstr_at_utf8(name).unwrap().into();
            self.ivars.insert(name_string, (offset, alignment));
        }
    }
}

impl ObjC {
    pub fn object_lookup_ivar(
        &self,
        mem: &Mem,
        obj: id,
        name: &String,
    ) -> Option<MutPtr<GuestUSize>> {
        let mut class = ObjC::read_isa(obj, mem);
        loop {
            let &ClassHostObject {
                superclass,
                ref ivars,
                ..
            } = self.borrow(class);
            if let Some((ivar_offset_ptr, _)) = ivars.get(name) {
                let ivar_offset = mem.read(*ivar_offset_ptr);
                let ivar_ptr = MutVoidPtr::from_bits(obj.to_bits() + ivar_offset);
                return Some(ivar_ptr.cast());
            } else if superclass == nil {
                return None;
            } else {
                class = superclass;
            }
        }
    }

    pub fn debug_all_class_ivars_as_strings(&self, class: Class) -> Vec<String> {
        let mut class = class;
        let mut ivars_strings = Vec::new();
        loop {
            let &ClassHostObject {
                superclass,
                ref ivars,
                ..
            } = self.borrow(class);
            let mut class_ivars_strings = ivars.keys().cloned().collect();
            ivars_strings.append(&mut class_ivars_strings);
            if superclass == nil {
                break;
            } else {
                class = superclass;
            }
        }
        ivars_strings
    }
}

// --- PROPERTY ACCESSORS ---

/// Core implementation for getting properties.
pub fn objc_getProperty(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
    atomic: bool,
) -> id {
    // Relax the assertion or change it to a warning
    if offset < 4 {
        log!("Warning: objc_getProperty called with unusually low offset: {}", offset);
    }

    if atomic {
        log_once!("TODO: Lock when atomic is set to true in objc_getProperty");
    }

    let ivar: MutPtr<id> = Ptr::from_bits(this.to_bits().checked_add_signed(offset).unwrap());
    env.mem.read(ivar)
}

pub fn objc_getProperty_atomic(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
) -> id {
    objc_getProperty(env, this, _cmd, offset, true)
}

pub fn objc_getProperty_nonatomic(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
) -> id {
    objc_getProperty(env, this, _cmd, offset, false)
}

/// Core implementation for setting properties.
pub fn objc_setProperty(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
    value: id,
    atomic: bool,
    should_copy: i8,
) {
    // Change the panic-inducing assert to a warning
    if offset < 4 {
        log!("Warning: objc_setProperty called with unusually low offset: {}", offset);
    }

    if atomic {
        log_once!("TODO: Lock when atomic is set to true in objc_setProperty");
    }

    let ivar: MutPtr<id> = Ptr::from_bits(this.to_bits().checked_add_signed(offset).unwrap());
    let old = env.mem.read(ivar);

    let void_null: MutVoidPtr = Ptr::null();
    let value: id = if value != nil {
        match should_copy {
            0 => retain(env, value),
            1 => msg![env; value copyWithZone:void_null],
            2 => msg![env; value mutableCopyWithZone:void_null],
            _ => panic!("Unknown \"should copy\" value: {should_copy}"),
        }
    } else {
        nil
    };
    env.mem.write(ivar, value);

    if old != nil {
        release(env, old);
    }
}

pub fn objc_setProperty_atomic(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
    value: id,
    should_copy: i8,
) {
    objc_setProperty(env, this, _cmd, offset, value, true, should_copy);
}

pub fn objc_setProperty_nonatomic(
    env: &mut Environment,
    this: id,
    _cmd: SEL,
    offset: GuestISize,
    value: id,
    should_copy: i8,
) {
    objc_setProperty(env, this, _cmd, offset, value, false, should_copy);
}

pub fn objc_copyStruct(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
    _atomic: bool,
    _hasStrong: bool,
) {
    env.mem.memmove(dest, src, size);
}

#[macro_export]
macro_rules! todo_objc_setter {
    ($this:ident, $($arg:tt)+) => {
        const _: () = {
            let bytes = _OBJC_CURRENT_SELECTOR.as_bytes();
            let starts_with_set =
                bytes.len() > 3 && bytes[0] == b's' && bytes[1] == b'e' && bytes[2] == b't';
            assert!(starts_with_set, "Selector does not start with set.");
        };
        log!(
            "TODO: [({}*) {:?} {}:{:?}]",
            _OBJC_CURRENT_CLASS,
            $this,
            _OBJC_CURRENT_SELECTOR,
            $($arg)+
        );
    };
}
pub use crate::todo_objc_setter;
