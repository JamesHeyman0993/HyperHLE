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
    env.objc.as_mut().unwrap().register_host_selector(string.into(), &mut env.mem)
}

pub fn NSStringFromClass(env: &mut Environment, class: Class) -> id {
    if class.is_null() {
        return nil;
    }
    let string = env.objc.as_mut().unwrap().get_class_name(class).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSClassFromString(env: &mut Environment, string: id) -> Class {
    if string == nil {
        return nil;
    }
    let class_name = ns_string::to_rust_string(env, string);

    match env.objc.as_mut().unwrap().get_class(&class_name) {
        Some(class) => class,
        None => {
            println!("NSClassFromString: Class '{}' not found, returning nil", class_name);
            nil
        }
    }
}

/// Internal helper to handle property logic.
fn perform_set_property(
    env: &mut Environment,
    _self: id,
    _cmd: SEL,
    new_value: id,
    offset: u32,
) {
    let ptr = _self + offset;
    
    // Use the environment's memory helper
    let old_value = env.mem.read_u32(ptr).unwrap_or(0);

    let objc = env.objc.as_mut().unwrap();
    let retain_sel = objc.sel_retain;
    let release_sel = objc.sel_release;

    // Retain new value
    if new_value != nil {
        objc.msg_send(new_value, retain_sel, &[], &mut env.mem);
    }

    // Write to memory
    if let Err(_) = env.mem.as_mut().unwrap().write_u32(ptr, new_value) {
        println!("Failed to write property at offset {}", offset);
    }

    // Release old value
    if old_value != nil {
        // Refresh objc reference since msg_send might need it
        let objc = env.objc.as_mut().unwrap();
        objc.msg_send(old_value, release_sel, &[], &mut env.mem);
    }
}

fn _objc_setProperty_nonatomic_copy(env: &mut Environment, _self: id, _cmd: SEL, new_value: id, offset: u32) {
    perform_set_property(env, _self, _cmd, new_value, offset);
}

fn objc_setProperty(env: &mut Environment, _self: id, _cmd: SEL, new_value: id, offset: u32) {
    perform_set_property(env, _self, _cmd, new_value, offset);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
    // Macro fix: List only the types of the arguments following the environment
    export_c_func!(_objc_setProperty_nonatomic_copy(id, SEL, id, u32)),
    export_c_func!(objc_setProperty(id, SEL, id, u32)),
];
