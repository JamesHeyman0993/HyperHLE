/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Objective-C runtime.
//!
//! Apple's [Programming with Objective-C](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ProgrammingWithObjectiveC/Introduction/Introduction.html)
//! is a useful introduction to the language from a user's perspective.
//! There are further resources in the child modules of this module, but they
//! are more implementation-specific.
//!
//! The strategy for this emulator will be to provide our own implementations of
//! an Objective-C runtime and libraries for it (Foundation etc). These
//! implementations will be "host code": Rust code forming part of the emulator,
//! not emulated code. The runtime will need to be able to handle classes that
//! originate from the guest app, classes defined by the host, and sometimes
//! classes that are both (considering Objective-C's support for inheritance,
//! categories and dynamic class editing).

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
pub use methods::{HostIMP, IMP};
pub use objects::{
    id, impl_HostObject_with_superclass, nil, AnyHostObject, HostObject, TrivialHostObject,
};
pub use properties::todo_objc_setter;
pub use selectors::{selector, SEL};

use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr};
use crate::objc::classes::___objc_personality_v0;
use crate::Environment;
// NEW: Import the execution trait required to use `.call_guest()` on the CPU context
use crate::abi::CallFromHost;

use classes::{ClassHostObject, FakeClass, UnimplementedClass};
use messages::{
    objc_msgSendSuper2, objc_msgSendSuper2_stret, objc_msgSend_stret, MsgSendSignature,
    MsgSendSuperSignature,
};
use methods::method_list_t;
use objects::{objc_object, HostObjectEntry};
use properties::{ivar_list_t, objc_copyStruct, objc_getProperty, objc_setProperty};
use selectors::sel_registerName;
use synchronization::{objc_sync_enter, objc_sync_exit};

/// Публичная обёртка над `messages::objc_msgSend` (которая `pub(super)`),
/// экспортируемая внутри крейта.
pub(crate) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    messages::objc_msgSend(env, receiver, selector)
}

/// Typedef for `NSZone *`. This is a [fossil type] found in the signature of
/// `allocWithZone:` and similar methods. Its value is always ignored.
///
/// [fossil type]: https://en.wiktionary.org/wiki/fossil_word
pub type NSZonePtr = crate::mem::MutVoidPtr;

/// Main type holding Objective-C runtime state.
pub struct ObjC {
    /// Known selectors (interned method name strings).
    selectors: HashMap<String, SEL>,

    /// Mapping of known (guest) object pointers to their host objects.
    ///
    /// If an object isn't in this map, we will consider it not to exist.
    objects: HashMap<id, HostObjectEntry>,

    /// Known classes.
    ///
    /// Look at the `isa` to get the metaclass for a class.
    classes: HashMap<String, Class>,

    /// Mutexes used in @synchronized blocks (objc_sync_enter/exit).
    sync_mutexes: HashMap<id, MutexId>,

    /// Temporary storage for optional type information when sending a message.
    /// Type information isn't part of the `objc_msgSend` ABI, so an alternative
    /// channel is needed.
    message_type_info: Option<(std::any::TypeId, &'static str)>,

    /// Set of classes that have already had `+initialize` sent to them
    /// (or were determined not to need it). Used to implement Apple's lazy
    /// `+initialize` dispatch contract:
    /// <https://developer.apple.com/documentation/objectivec/nsobject/1418639-initialize>
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

    /// Returns the name of a selector, panicking if it is unknown.
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
    // We don't use these in our Objective-C runtime, but exporting useless
    // symbols for these silences the warning about the unhandled relocation,
    // and avoids a linker error for the integration tests.
    ("__objc_empty_vtable", HostConstant::NullPtr),
    ("__objc_empty_cache", HostConstant::NullPtr),
    ("_OBJC_EHTYPE_$_NSException", HostConstant::NullPtr),
    ("_OBJC_EHTYPE_id", HostConstant::NullPtr),
    // FIXED: Added companion constant mapping for the unwinder routine to prevent dyld relocation dropouts
    ("___objc_personality_v0", HostConstant::NullPtr),
    // `NSObject`'s only ivar (`isa`) lives at offset 0 in the object layout
    // on 32-bit iOS, so resolving the ivar-offset symbol to a 4-byte zero
    // gives any binary that does `obj + _OBJC_IVAR_$_NSObject.isa` the
    // correct address (i.e. the object base).
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

/// Block support is iOS 4+, but it seems like Block Runtime Helpers
/// could still be called on even if minimal iOS version is set to 3.x?
///
/// ref. <https://clang.llvm.org/docs/Block-ABI-Apple.html#runtime-helper-functions>
fn _Block_object_dispose(_env: &mut Environment, object: ConstVoidPtr, flags: i32) {
    // `BLOCK_FIELD_IS_BYREF` flag defines an on stack structure holding
    // the __block variable. It is _probably_ safe to ignore.
    // TODO: properly implement for block support
    assert!(flags == 8); // BLOCK_FIELD_IS_BYREF
    log!(
        "Warning: Ignoring _Block_object_dispose({:?}, BLOCK_FIELD_IS_BYREF)",
        object
    );
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
    
    // Use from_bits to reconstruct the Class pointer from the raw integer bits
    let class_type = Class::from_bits(class_ptr.to_bits());
    let class_name = env.objc.get_class_name(class_type);
    
    log!("HyperHLE: Intercepted _objc_alloc optimization invocation for class: {}", class_name);
    
    // Execute standard touchHLE object instance allocation: [class_ptr alloc]
    let allocated_object: id = crate::objc::msg![env; class_ptr alloc];
    allocated_object
}

/// Dynamic implementation for class_getName(Class cls) -> const char *
fn class_getName(env: &mut Environment, class_ptr: id) -> ConstPtr<u8> {
    if class_ptr == nil {
        return ConstPtr::from_bits(0);
    }
    
    let class_type = Class::from_bits(class_ptr.to_bits());
    let class_name = env.objc.get_class_name(class_type);
    
    // Convert string to a null-terminated C-string array inside guest context
    let mut name_bytes = class_name.as_bytes().to_vec();
    name_bytes.push(0);
    
    let len = name_bytes.len() as u32;
    let guest_alloc = env.mem.alloc(len);
    env.mem.bytes_at_mut(guest_alloc.cast(), len).copy_from_slice(&name_bytes);
    
    guest_alloc.cast().cast_const()
}

/// Stub implementation for protocol_getName(Protocol *p) -> const char *
fn protocol_getName(env: &mut Environment, protocol_ptr: id) -> ConstPtr<u8> {
    log!("Warning: protocol_getName called for address {:?} — returning fallback descriptor", protocol_ptr);
    let mock_name = "FakedProtocol\0";
    let len = mock_name.len() as u32;
    let guest_alloc = env.mem.alloc(len);
    env.mem.bytes_at_mut(guest_alloc.cast(), len).copy_from_slice(mock_name.as_bytes());
    
    guest_alloc.cast().cast_const()
}

/// Dynamic implementation for objc_lookUpClass(const char *name) -> Class
fn objc_lookUpClass(env: &mut Environment, name_ptr: ConstPtr<u8>) -> id {
    if name_ptr.is_null() {
        return nil;
    }
    
    // Safely pull bytes from emulated layout up to the null-terminator
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
    
    // Cast layout structure cleanly back to dynamic guest raw identifier
    resolved_class.cast::<crate::objc::objects::objc_object>()
}

/// Functional replacement execution engine for Grand Central Dispatch `dispatch_once_f`
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

    // Apple Specification status value: -1 means initialization complete. 
    // 0 means unexecuted block sequence layout.
    if predicate_val != -1 {
        // Set context flag immediately to lock execution path
        env.mem.write(predicate_ptr, -1);

        log!("HyperHLE: dispatch_once_f invoking guest initialization callback at {:?}", function_ptr);

        // Bring the CallFromHost execution trait into local scope
        use crate::abi::CallFromHost;

        // Pass the user context pointer as the sole argument inside a tuple
        let args = (context,);

        // Execute the guest code directly via the pointer abstraction layer
        let _: () = function_ptr.call_from_host(env, args);
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
    
    // FIXED: Shifted token signature counts to single guest arguments (_)
    export_c_func!(class_getName(_)),
    export_c_func!(protocol_getName(_)),
    export_c_func!(objc_lookUpClass(_)),

    // NEW: Added missing multi-threading initialization synchronization engine hooks
    export_c_func!(dispatch_once_f(_, _, _)),
];
