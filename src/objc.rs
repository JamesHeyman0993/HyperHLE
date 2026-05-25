/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Objective-C runtime.

use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant, HostDylib};
use crate::MutexId;
use std::collections::{HashMap, HashSet};

mod classes;
mod messages;
mod methods;
mod objects;
mod properties;
mod selectors;
mod synchronization;

pub use classes::{
    class_getInstanceMethod, class_getInstanceSize, class_getProperty, class_getSuperclass,
    class_replaceMethod, method_getImplementation, method_getTypeEncoding,
    method_setImplementation, objc_autoreleasePoolPop, objc_autoreleasePoolPush,
    objc_autoreleaseReturnValue, objc_begin_catch, objc_classes, objc_end_catch,
    objc_exception_throw, objc_getClass, objc_getMetaClass, objc_release, objc_retain,
    objc_retainAutoreleaseReturnValue, objc_retainAutoreleasedReturnValue,
    objc_setProperty_nonatomic, object_getClass, object_getClassName, Class, ClassExports,
    ClassTemplate,
};
pub use messages::{
    autorelease, msg, msg_class, msg_send, msg_send_no_type_checking, msg_send_super2, msg_super,
    objc_super, release, retain,
};
pub use methods::{HostIMP, IMP, method_list_t};
pub use objects::{
    id, impl_HostObject_with_superclass, nil, objc_object, AnyHostObject, HostObject, TrivialHostObject,
}; // FIXED: Exported objc_object publicly here
pub use properties::todo_objc_setter;
pub use selectors::{selector, SEL};

use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr};
use crate::objc::classes::___objc_personality_v0;
use crate::Environment;
use crate::abi::CallFromHost;

use classes::{ClassHostObject, FakeClass, UnimplementedClass};
use messages::{
    objc_msgSendSuper2, objc_msgSendSuper2_stret, objc_msgSend_stret, MsgSendSignature,
    MsgSendSuperSignature,
};
use objects::HostObjectEntry;
use properties::{ivar_list_t, objc_copyStruct, objc_getProperty, objc_setProperty};
use selectors::sel_registerName;
use synchronization::{objc_sync_enter, objc_sync_exit};

pub(crate) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    messages::objc_msgSend(env, receiver, selector)
}

pub type NSZonePtr = crate::mem::MutVoidPtr;

pub struct ObjC {
    selectors: HashMap<String, SEL>,
    objects: HashMap<id, HostObjectEntry>,
    classes: HashMap<String, Class>,
    sync_mutexes: HashMap<id, MutexId>,
    message_type_info: Option<(std::any::TypeId, &'static str)>,
    pub(super) initialized_classes: HashSet<Class>,
}

impl ObjC {
    pub fn new() -> ObjC {
        ObjC {
            selectors: HashMap::new(),
            objects: HashMap::new(),
            classes: HashMap::new(),
            sync_mutexes: HashMap::new(),
            message_type_info: None,
            initialized_classes: HashSet::new(),
        }
    }

    pub fn get_selector_name(&self, sel: SEL) -> &str {
        self.selectors
            .iter()
            .find(|(_k, v)| **v == sel)
            .map(|(k, _v)| k.as_str())
            .expect("get_selector_name: unknown selector")
    }
}

pub const DYLIB: HostDylib = HostDylib {
    path: "/usr/lib/libobjc.A.dylib",
    aliases: &["/usr/lib/libobjc.dylib"],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};

const CONSTANTS: ConstantExports = &[
    ("__objc_empty_vtable", HostConstant::NullPtr),
    ("__objc_empty_cache", HostConstant::NullPtr),
    ("_OBJC_EHTYPE_$_NSException", HostConstant::NullPtr),
    ("_OBJC_EHTYPE_id", HostConstant::NullPtr),
    ("___objc_personality_v0", HostConstant::NullPtr),
    ("_OBJC_IVAR_$_NSObject.isa", HostConstant::NullPtr),
    ("_kCFTypeArrayCallBacks", HostConstant::NullPtr),
    ("_NSHTTPCookieDomain", HostConstant::NSString("NSHTTPCookieDomain")),
    ("_NSHTTPCookieValue", HostConstant::NSString("NSHTTPCookieValue")),
    ("_NSHTTPCookieName", HostConstant::NSString("NSHTTPCookieName")),
    ("_NSHTTPCookiePath", HostConstant::NSString("NSHTTPCookiePath")),
    ("_NSKeyValueChangeNewKey", HostConstant::NSString("new")),
];

fn _Block_object_dispose(_env: &mut Environment, object: ConstVoidPtr, flags: i32) {
    assert!(flags == 8);
    log!("Warning: Ignoring _Block_object_dispose({:?}, BLOCK_FIELD_IS_BYREF)", object);
}

fn objc_retainAutorelease(env: &mut Environment, obj: id) -> id {
    if obj != nil {
        retain(env, obj);
        autorelease(env, obj);
    }
    obj
}

fn objc_alloc(env: &mut Environment, class_ptr: id) -> id {
    if class_ptr == nil {
        return nil;
    }
    let class_type = Class::from_bits(class_ptr.to_bits());
    let class_name = env.objc.get_class_name(class_type);
    log!("HyperHLE: Intercepted _objc_alloc optimization invocation for class: {}", class_name);
    let allocated_object: id = crate::objc::msg![env; class_ptr alloc];
    allocated_object
}

fn class_getName(env: &mut Environment, class_ptr: id) -> ConstPtr<u8> {
    if class_ptr == nil {
        return ConstPtr::from_bits(0);
    }
    let class_type = Class::from_bits(class_ptr.to_bits());
    let class_name = env.objc.get_class_name(class_type);
    let mut name_bytes = class_name.as_bytes().to_vec();
    name_bytes.push(0);
    let len = name_bytes.len() as u32;
    let guest_alloc = env.mem.alloc(len);
    env.mem.bytes_at_mut(guest_alloc.cast(), len).copy_from_slice(&name_bytes);
    guest_alloc.cast().cast_const()
}

fn protocol_getName(env: &mut Environment, protocol_ptr: id) -> ConstPtr<u8> {
    log!("Warning: protocol_getName called for address {:?} — returning fallback descriptor", protocol_ptr);
    let mock_name = "FakedProtocol\0";
    let len = mock_name.len() as u32;
    let guest_alloc = env.mem.alloc(len);
    env.mem.bytes_at_mut(guest_alloc.cast(), len).copy_from_slice(mock_name.as_bytes());
    guest_alloc.cast().cast_const()
}

fn objc_lookUpClass(env: &mut Environment, name_ptr: ConstPtr<u8>) -> id {
    if name_ptr.is_null() {
        return nil;
    }
    let mut bytes = Vec::new();
    let mut offset = 0;
    loop {
        let current_byte: u8 = env.mem.read(ConstPtr::from_bits(name_ptr.to_bits() + offset));
        if current_byte == 0 {
            break;
        }
        bytes.push(current_byte);
        offset += 1;
    }
    let class_name = String::from_utf8_lossy(&bytes).into_owned();
    let resolved_class = env.objc.get_known_class(&class_name, &mut env.mem);
    log!("HyperHLE: objc_lookUpClass linked descriptor for: {}", class_name);
    resolved_class.cast::<objc_object>()
}

fn dispatch_once_f(
    env: &mut Environment,
    predicate_ptr: MutPtr<i32>,
    context: MutVoidPtr,
    function_ptr: ConstPtr<u8>,
) {
    if predicate_ptr.is_null() || function_ptr.is_null() {
        return;
    }
    let predicate_val: i32 = env.mem.read(predicate_ptr);
    if predicate_val != -1 {
        env.mem.write(predicate_ptr, -1);
        log!("HyperHLE: dispatch_once_f invoking guest initialization callback at {:?}", function_ptr);
        use crate::abi::CallFromHost;
        type GuestInitFn = fn(&mut Environment, MutVoidPtr);
        let target_address = function_ptr.to_bits() as usize;
        let guest_call: GuestInitFn = unsafe { std::mem::transmute(target_address) };
        let args = (context,);
        let _: () = guest_call.call_from_host(env, args);
    }
}

fn objc_setProperty_nonatomic_copy(
    env: &mut Environment,
    obj: id,
    _cmd: id,
    offset: usize,
    value: id,
) {
    if obj != nil {
        // FIXED: Replaced non-existent wrapping_add method on Ptr structure with plain address offset math addition
        let target_address = ConstPtr::<u8>::from_bits(obj.to_bits() + offset as u32);
        env.mem.write(target_address.cast_mut(), value);
        retain(env, value);
        log_dbg!("HyperHLE: objc_setProperty_nonatomic_copy stored value reference at offset {}", offset);
    }
}

const FUNCTIONS: FunctionExports = &[
    export_c_func!(objc_msgSend(_, _)),
    export_c_func!(objc_msgSend_stret(_, _, _)),
    export_c_func!(objc_msgSendSuper2_stret(_, _)),
    export_c_func!(objc_msgSendSuper2(_, _)),
    export_c_func!(objc_alloc(_)),
    export_c_func!(objc_getProperty(_, _, _, _)),
    export_c_func!(objc_setProperty(_, _, _, _, _, _)),
    export_c_func!(objc_copyStruct(_, _, _, _, _)),
    export_c_func!(objc_sync_enter(_)),
    export_c_func!(objc_sync_exit(_)),
    export_c_func!(sel_registerName(_)),
    export_c_func!(objc_getClass(_)),
    export_c_func!(objc_getMetaClass(_, _)),
    export_c_func!(object_getClassName(_)),
    export_c_func!(object_getClass(_)),
    export_c_func!(objc_retainAutoreleasedReturnValue(_)),
    export_c_func!(objc_autoreleaseReturnValue(_)),
    export_c_func!(objc_retainAutoreleaseReturnValue(_)),
    export_c_func!(objc_autoreleasePoolPush(_)),
    export_c_func!(objc_autoreleasePoolPop(_)),
    export_c_func!(objc_retain(_)),
    export_c_func!(objc_release(_)),
    export_c_func!(objc_retainAutorelease(_)),
    export_c_func!(objc_setProperty_nonatomic(_)),
    // FIXED: expanded macro signature to explicitly match the 4 trailing argument slots of the function
    export_c_func!(objc_setProperty_nonatomic_copy(_, _, _, _, _)), 
    export_c_func!(objc_exception_throw(_)),
    export_c_func!(objc_begin_catch(_)),
    export_c_func!(objc_end_catch(_)),
    export_c_func!(class_getSuperclass(_)),
    export_c_func!(class_getProperty(_, _)),
    export_c_func!(class_getInstanceSize(_, _)),
    export_c_func!(class_getInstanceMethod(_, _)),
    export_c_func!(class_replaceMethod(_, _)),
    export_c_func!(method_getImplementation(_, _)),
    export_c_func!(method_setImplementation(_, _)),
    export_c_func!(method_getTypeEncoding(_, _)),
    export_c_func!(_Block_object_dispose(_, _)),
    export_c_func!(___objc_personality_v0(_, _, _, _, _)),
    export_c_func!(class_getName(_)),
    export_c_func!(protocol_getName(_)),
    export_c_func!(objc_lookUpClass(_)),
    export_c_func!(dispatch_once_f(_, _, _)),
];
