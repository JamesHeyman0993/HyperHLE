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
    let class_name = ns_string::to_rust_string(env, string);

    // Modified to be safer: if the class isn't found, return nil so the 
    // guest app can handle the absence gracefully instead of crashing/looping.
    match env.objc.get_class(&class_name) {
        Some(class) => class,
        None => {
            log::warn!("NSClassFromString: Class '{}' not found, returning nil", class_name);
            nil
        }
    }
}

/// Implementation for _objc_setProperty and its variants.
/// This handles the logic of releasing the old value and retaining/copying the new one.
fn _objc_setProperty_nonatomic_copy(
    env: &mut Environment,
    _self: id,
    _cmd: SEL,
    new_value: id,
    offset: u32,
) {
    let ptr = _self + offset;

    // Read the current value stored at the offset
    let old_value = env.mem.read_u32(ptr).unwrap_or(0);

    // If new_value isn't null, retain it. 
    // (Simplification: treating 'copy' as 'retain' for now to ensure stability)
    if new_value != nil {
        env.objc.msg_send(new_value, env.objc.sel_retain, &[], &mut env.mem);
    }

    // Write the new object pointer to the instance variable
    if let Err(e) = env.mem.write_u32(ptr, new_value) {
        log::error!("Failed to write property at offset {}: {:?}", offset, e);
    }

    // Release the old value
    if old_value != nil {
        env.objc.msg_send(old_value, env.objc.sel_release, &[], &mut env.mem);
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    // Fixes for the Doodle Jump "unimplemented" crashes:
    export_c_func!(_objc_setProperty_nonatomic_copy(env, _self, _cmd, new_value, offset)),
    export_c_func!(objc_setProperty(env, _self, _cmd, new_value, offset) -> _objc_setProperty_nonatomic_copy),
];
