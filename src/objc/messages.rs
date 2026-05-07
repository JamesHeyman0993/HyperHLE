/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 */
//!
//! Handling of Objective-C messaging (`objc_msgSend` and friends).

use super::{id, nil, Class, ObjC, IMP, SEL};
use crate::abi::{CallFromHost, GuestRet};
use crate::mem::{ConstPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::any::TypeId;

fn ensure_class_initialized(env: &mut Environment, class_to_init: Class) {
    if class_to_init == nil {
        return;
    }
    if env.objc.initialized_classes.contains(&class_to_init) {
        return;
    }

    let superclass = {
        let Some(host_object) = env.objc.get_host_object(class_to_init) else {
            env.objc.initialized_classes.insert(class_to_init);
            return;
        };
        if let Some(co) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
            co.superclass
        } else {
            env.objc.initialized_classes.insert(class_to_init);
            return;
        }
    };
    ensure_class_initialized(env, superclass);

    if !env.objc.initialized_classes.insert(class_to_init) {
        return;
    }

    let metaclass = ObjC::read_isa(class_to_init, &env.mem);
    let Some(sel_initialize) = env.objc.lookup_selector("initialize") else {
        return;
    };
    if !env.objc.class_has_method(metaclass, sel_initialize) {
        return;
    }

    let saved_r0_r3 = [
        env.cpu.regs()[0],
        env.cpu.regs()[1],
        env.cpu.regs()[2],
        env.cpu.regs()[3],
    ];
    log_dbg!("Dispatching +[{:?} initialize]", class_to_init);
    let _: () = msg_send_no_type_checking(env, (class_to_init, sel_initialize));
    let regs = env.cpu.regs_mut();
    regs[0..4].copy_from_slice(&saved_r0_r3);
}

#[allow(non_snake_case)]
fn objc_msgSend_inner(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
    super2: Option<Class>,
    tolerate_type_mismatch: bool,
) {
    const MAX_DEPTH: usize = 128;
    thread_local! {
        static DISPATCH_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    let depth = DISPATCH_DEPTH.with(|d| {
        let new = d.get() + 1;
        d.set(new);
        new
    });
    struct DepthGuard;
    impl Drop for DepthGuard {
        fn drop(&mut self) {
            DISPATCH_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
    let _guard = DepthGuard;
    if depth > MAX_DEPTH {
        log!("Warning: recursion limit exceeded for \"{}\"", selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let message_type_info = env.objc.message_type_info.take();

    if receiver == nil {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if orig_class == nil {
        log!("Warning: receiver {:?} has nil isa! Ignoring \"{}\".", receiver, selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if super2.is_none() {
        if let Some(host_object) = env.objc.get_host_object(orig_class) {
            if let Some(co) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
                let class_to_init = if co.is_metaclass { receiver } else { orig_class };
                ensure_class_initialized(env, class_to_init);
            }
        }
    }

    let mut class = orig_class;
    loop {
        if class == nil {
            log!("Warning: {:?} does not respond to selector \"{}\"!", receiver, selector.as_str(&env.mem));
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }

        let Some(host_object) = env.objc.get_host_object(class) else {
            log!("Warning: class {:?} has no host object — stopping dispatch", class);
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        };

        if let Some(co) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
            let superclass = co.superclass;
            let methods = &co.methods;
            let name = &co.name;

            if super2.is_some() && class == orig_class {
                class = superclass;
                continue;
            }

            if let Some(imp) = methods.get(&selector) {
                match imp {
                    IMP::Host(host_imp) => {
                        if let Some((sent_type_id, sent_type_desc)) = message_type_info {
                            let (expected_type_id, expected_type_desc) = host_imp.type_info();
                            if sent_type_id != expected_type_id && !tolerate_type_mismatch {
                                log_dbg!("Type mismatch for {}", selector.as_str(&env.mem));
                            }
                        }
                        host_imp.call_from_guest(env)
                    }
                    IMP::Guest(guest_imp) => guest_imp.call_without_pushing_stack_frame(env),
                }
                return;
            } else {
                class = superclass;
            }
        } else {
            // Handle FakeClass or Unimplemented
            log!("Warning: Dispatching to Fake/Unimplemented class at {:?}", class);
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }
    }
}

pub(super) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, false)
}

pub(crate) fn _touchHLE_objc_msgSend_tolerant(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, true)
}

pub(super) fn objc_msgSend_stret(env: &mut Environment, _stret: MutVoidPtr, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, false)
}

pub(crate) fn _touchHLE_objc_msgSend_stret_tolerant(env: &mut Environment, _stret: MutVoidPtr, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, true)
}

#[repr(C, packed)]
pub struct objc_super {
    pub receiver: id,
    pub class: Class,
}
unsafe impl SafeRead for objc_super {}

pub(super) fn objc_msgSendSuper2(env: &mut Environment, super_ptr: ConstPtr<objc_super>, selector: SEL) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);
    objc_msgSend_inner(env, receiver, selector, Some(class), false)
}

pub trait MsgSendSignature: 'static {
    fn type_info() -> (TypeId, &'static str) {
        (TypeId::of::<Self>(), "type info")
    }
}

// Support for standard arg counts
impl<R: 'static, P1: 'static, P2: 'static, P3: 'static, P4: 'static, P5: 'static, P6: 'static, P7: 'static> MsgSendSignature for (R, (id, SEL, P1, P2, P3, P4, P5, P6, P7)) {}
impl<R: 'static, P1: 'static, P2: 'static, P3: 'static, P4: 'static, P5: 'static, P6: 'static, P7: 'static, P8: 'static> MsgSendSignature for (R, (id, SEL, P1, P2, P3, P4, P5, P6, P7, P8)) {}

pub fn msg_send<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    // NUCLEAR SAFETY
    let receiver_ptr = &args as *const P as *const id;
    unsafe { if *receiver_ptr == nil { return R::from_uintptr(0); } }

    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    if R::SIZE_IN_MEM.is_some() {
        (objc_msgSend_stret as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (objc_msgSend as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

pub fn msg_send_no_type_checking<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    let receiver_ptr = &args as *const P as *const id;
    unsafe { if *receiver_ptr == nil { return R::from_uintptr(0); } }

    if R::SIZE_IN_MEM.is_some() {
        (_touchHLE_objc_msgSend_stret_tolerant as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (_touchHLE_objc_msgSend_tolerant as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

pub trait MsgSendSuperSignature: 'static {
    type WithoutSuper: MsgSendSignature;
}

impl<R: 'static, P1: 'static, P2: 'static, P3: 'static, P4: 'static, P5: 'static, P6: 'static, P7: 'static> MsgSendSuperSignature for (R, (ConstPtr<objc_super>, SEL, P1, P2, P3, P4, P5, P6, P7)) {
    type WithoutSuper = (R, (id, SEL, P1, P2, P3, P4, P5, P6, P7));
}

pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL)).call_from_host(env, args)
}

#[macro_export]
macro_rules! msg {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let sel_str = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel_str).expect("Unknown selector");
            let args = ($receiver, sel, $($arg1, $($argn),*)?);
            $crate::objc::msg_send($env, args)
        }
    }
}

pub fn retain(env: &mut Environment, object: id) -> id {
    if object == nil { return nil; }
    msg![env; object retain]
}

pub fn release(env: &mut Environment, object: id) {
    if object == nil { return; }
    msg![env; object release]
}

pub fn autorelease(env: &mut Environment, object: id) -> id {
    if object == nil { return nil; }
    msg![env; object autorelease]
    }
