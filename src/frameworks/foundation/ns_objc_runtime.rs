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

    match env.objc.get_class(&name) {
        Some(class) => class,
        None => nil,
    }
}

/// The actual logic for setting properties
fn objc_setProperty(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    let ptr = _self + offset;
    if let Err(_) = env.mem.write(ptr, val) {
        log::error!("objc_setProperty failed at offset {}", offset);
    }
}

/// Wrapper for the nonatomic copy version
fn _objc_setProperty_nonatomic_copy(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    objc_setProperty(env, _self, _cmd, val, offset);
}

/// Wrapper for the atomic version
fn objc_setProperty_atomic(env: &mut Environment, _self: id, _cmd: SEL, val: id, offset: u32) {
    objc_setProperty(env, _self, _cmd, val, offset);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    export_c_func!(objc_setProperty(id, SEL, id, u32)),
    export_c_func!(_objc_setProperty_nonatomic_copy(id, SEL, id, u32)),
    export_c_func!(objc_setProperty_atomic(id, SEL, id, u32)),
];
