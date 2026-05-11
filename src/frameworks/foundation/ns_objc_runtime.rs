//! Things from `NSObjCRuntime.h`.

use super::ns_string;
use crate::dyld::{export_c_func, FunctionExports};
use crate::objc::{id, nil, Class, SEL};
use crate::Environment;

fn NSStringFromSelector(env: &mut Environment, selector: SEL) -> id {
    // TODO: caching?
    let string = selector.as_str(&env.mem).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSSelectorFromString(env: &mut Environment, string: id) -> SEL {
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);
    env.objc.register_host_selector(string.into(), &mut env.mem)
}

pub fn NSStringFromClass(env: &mut Environment, class: Class) -> id {
    if class.is_null() {
        return nil;
    }
    // TODO: caching?
    let string = env.objc.get_class_name(class).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSClassFromString(env: &mut Environment, string: id) -> Class {
    if string == nil {
        return nil;
    }
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);

    // While this method is supposed to return nil if the class is not found,
    // touchHLE is missing many classes that apps might expect to be present,
    // so this could be troublesome. So, let's use get_known_class, which panics
    // when it can't find the class. We could except certain classes or apps if
    // we need to.
    env.objc.get_known_class(&string, &mut env.mem)
}

// --- ENGINE GLUE FIXES ---
// These functions allow the game engine to link its script variables to memory.
// This is required for the "Start" button and other UI logic to function.

fn objc_setProperty(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    let dest_ptr = _self + offset;
    // We use write_ptr because we are writing an 'id' (a pointer) 
    // into a specific memory address.
    if let Err(e) = env.mem.write_ptr(dest_ptr, val) {
        log::error!("objc_setProperty failed to write to offset {}: {:?}", offset, e);
    }
}

fn _objc_setProperty_nonatomic_copy(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    objc_setProperty(env, _self, _cmd, val, offset);
}

fn objc_setProperty_atomic(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    objc_setProperty(env, _self, _cmd, val, offset);
}

// -------------------------

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    // Missing Property Setter exports:
    export_c_func!(objc_setProperty(id, SEL, id, u32)),
    export_c_func!(_objc_setProperty_nonatomic_copy(id, SEL, id, u32)),
    export_c_func!(objc_setProperty_atomic(id, SEL, id, u32)),
];
