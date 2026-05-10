/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
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
    class_getInstanceMethod, class_getInstanceSize, class_getSuperclass, class_replaceMethod,
    method_getImplementation, method_getTypeEncoding, method_setImplementation,
    objc_autoreleasePoolPop, objc_autoreleasePoolPush, objc_autoreleaseReturnValue,
    objc_begin_catch, objc_classes, objc_end_catch, objc_exception_throw, objc_getClass,
    objc_getMetaClass, objc_release, objc_retain, objc_retainAutoreleaseReturnValue,
    objc_retainAutoreleasedReturnValue, objc_setProperty_nonatomic, object_getClass,
    object_getClassName, Class, ClassExports, ClassTemplate,
};
pub use messages::{
    autorelease, msg, msg_class, msg_send, msg_send_no_type_checking, msg_send_super2, msg_super,
    objc_super, release, retain,
};
pub use methods::{HostIMP, IMP};
pub use objects::{
    id, impl_HostObject_with_superclass, nil, AnyHostObject, HostObject, TrivialHostObject,
};
pub use properties::{
    objc_getProperty_atomic, objc_getProperty_nonatomic, objc_setProperty_atomic,
    todo_objc_setter,
};
pub use selectors::{selector, SEL};

use crate::mem::ConstVoidPtr;
use crate::objc::classes::___objc_personality_v0;
use crate::Environment;
use classes::{ClassHostObject, FakeClass, UnimplementedClass};
use messages::{
    objc_msgSendSuper2, objc_msgSendSuper2_stret, objc_msgSend_stret, MsgSendSignature,
    MsgSendSuperSignature,
};
use methods::method_list_t;
use objects::{objc_object, HostObjectEntry};
use properties::{
    ivar_list_t, objc_copyStruct, objc_getProperty, objc_setProperty,
};
use selectors::sel_registerName;
use synchronization::{objc_sync_enter, objc_sync_exit};

/// Публичная обёртка над `messages::objc_msgSend` (которая `pub(super)`),
/// экспортируемая внутри крейта.
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
    ("_OBJC_IVAR_$_NSObject.isa", HostConstant::NullPtr),
    ("_kCFTypeArrayCallBacks", HostConstant::NullPtr),
    (
        "_NSHTTPCookieDomain",
        HostConstant::NSString("NSHTTPCookieDomain"),
    ),
    (
        "_NSHTTPCookieValue",
        HostConstant::NSString("NSHTTPCookieValue"),
    ),
    (
        "_NSHTTPCookieName",
        HostConstant::NSString("NSHTTPCookieName"),
    ),
    (
        "_NSHTTPCookiePath",
        HostConstant::NSString("NSHTTPCookiePath"),
    ),
    ("_NSKeyValueChangeNewKey", HostConstant::NSString("new")),
];

fn _Block_object_dispose(_env: &mut Environment, object: ConstVoidPtr, flags: i32) {
    assert!(flags == 8); // BLOCK_FIELD_IS_BYREF
    log!(
        "Warning: Ignoring _Block_object_dispose({:?}, BLOCK_FIELD_IS_BYREF)",
        object
    );
}

const FUNCTIONS: FunctionExports = &[
    export_c_func!(objc_msgSend(_, _)),
    export_c_func!(objc_msgSend_stret(_, _, _)),
    export_c_func!(objc_msgSendSuper2_stret(_, _)),
    export_c_func!(objc_msgSendSuper2(_, _)),
    
    // Property Getters (4 arguments: env, this, cmd, offset)
    export_c_func!(objc_getProperty(_, _, _, _)),
    export_c_func!(objc_getProperty_atomic(_, _, _, _)),
    export_c_func!(objc_getProperty_nonatomic(_, _, _, _)),
    
    // Property Setters (6 arguments: env, this, cmd, offset, value, atomic/copy)
    export_c_func!(objc_setProperty(_, _, _, _, _, _)),
    export_c_func!(objc_setProperty_atomic(_, _, _, _, _, _)),
    export_c_func!(objc_setProperty_nonatomic(_, _, _, _, _, _)),
    
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
    export_c_func!(objc_exception_throw(_)),
    export_c_func!(objc_begin_catch(_)),
    export_c_func!(objc_end_catch(_)),
    export_c_func!(class_getSuperclass(_)),
    export_c_func!(class_getInstanceSize(_, _)),
    export_c_func!(class_getInstanceMethod(_, _)),
    export_c_func!(class_replaceMethod(_, _)),
    export_c_func!(method_getImplementation(_, _)),
    export_c_func!(method_setImplementation(_, _)),
    export_c_func!(method_getTypeEncoding(_, _)),
    export_c_func!(_Block_object_dispose(_, _)),
    export_c_func!(___objc_personality_v0(_, _, _, _, _)),
];
