//! Things from `NSObjCRuntime.h`.

use super::ns_string;
use crate::dyld::{export_c_func, FunctionExports};
use crate::objc::{id, nil, Class, SEL};
use crate::Environment;

fn NSStringFromSelector(env: &mut Environment, selector: SEL) -> id {
    let string = selector.as_str(&env.mem).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSSelectorFromString(env: &mut Environment, string: id) -> SEL {
    let string = ns_string::to_rust_string(env, string);
    env.objc.register_host_selector(string.into(), &mut env.mem)
}

pub fn NSStringFromClass(env: &mut Environment, class: Class) -> id {
    if class.is_null() {
        return nil;
    }
    let string = env.objc.get_class_name(class).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSClassFromString(env: &mut Environment, string: id) -> Class {
    if string == nil {
        return nil;
    }
    let name = ns_string::to_rust_string(env, string);

    // FIX: Use get_class and return nil if not found.
    // This prevents the "get_known_class" panic loop.
    match env.objc.get_class(&name) {
        Some(class) => class,
        None => nil,
    }
}

/// Helper for property setters used by many game engines.
fn objc_setProperty(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    let ptr = _self + offset;
    // Write the value to memory. If it fails, we just log it.
    if let Err(_) = env.mem.write(ptr, val) {
        log::error!("objc_setProperty failed at offset {}", offset);
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    // These link the game's internal variables to the emulator's memory
    export_c_func!(objc_setProperty(id, SEL, id, u32)),
    export_c_func!(_objc_setProperty_nonatomic_copy(id, SEL, id, u32) -> objc_setProperty),
    export_c_func!(objc_setProperty_atomic(id, SEL, id, u32) -> objc_setProperty),
];
