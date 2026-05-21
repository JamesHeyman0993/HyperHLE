/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Thread-specific data keys.

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstVoidPtr, MutPtr, MutVoidPtr, Ptr};
use crate::{Environment, ThreadId};
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    /// The `pthread_key_t` value, with 1 subtracted, is the index into this
    /// vector. The tuple contains the map of thread-specific data pointers plus
    /// the destructor pointer.
    keys: Vec<(HashMap<ThreadId, MutVoidPtr>, GuestFunction)>,
}

fn get_state(env: &mut Environment) -> &mut State {
    &mut env.libc_state.pthread.key
}

type pthread_key_t = u32;

fn pthread_key_create(
    env: &mut Environment,
    key_ptr: MutPtr<pthread_key_t>,
    destructor: GuestFunction, // void (*destructor)(void *), may be NULL
) -> i32 {
    let idx = get_state(env).keys.len();
    let key: pthread_key_t = (idx + 1).try_into().unwrap();
    get_state(env).keys.push((HashMap::new(), destructor));
    env.mem.write(key_ptr, key);
    0 // success
}

fn pthread_getspecific(env: &mut Environment, key: pthread_key_t) -> MutVoidPtr {
    // Gracefully handle uninitialized or invalid keys (0 or out of bounds) instead of unwrapping None
    let Some(sub_key) = key.checked_sub(1) else {
        log!("Warning: pthread_getspecific called with uninitialized key (0). Returning null.");
        return Ptr::null();
    };
    
    let idx: usize = match sub_key.try_into() {
        Ok(val) => val,
        Err(_) => return Ptr::null(),
    };

    let state = get_state(env);
    if idx >= state.keys.len() {
        log!("Warning: pthread_getspecific called with out-of-bounds key ({}). Returning null.", key);
        return Ptr::null();
    }

    let current_thread = env.current_thread;
    state.keys[idx]
        .0
        .get(&current_thread)
        .copied()
        .unwrap_or(Ptr::null())
}

fn pthread_setspecific(env: &mut Environment, key: pthread_key_t, value: ConstVoidPtr) -> i32 {
    // Gracefully handle uninitialized or invalid keys instead of panicking
    let Some(sub_key) = key.checked_sub(1) else {
        log!("Warning: pthread_setspecific called with uninitialized key (0). Ignoring set request.");
        return 0; // Return 0 to keep the engine moving forward safely
    };

    let idx: usize = match sub_key.try_into() {
        Ok(val) => val,
        Err(_) => return 22, // EINVAL (Invalid argument)
    };

    let state = get_state(env);
    if idx >= state.keys.len() {
        log!("Warning: pthread_setspecific called with out-of-bounds key ({}).", key);
        return 22; // EINVAL
    }

    let current_thread = env.current_thread;
    state.keys[idx]
        .0
        .insert(current_thread, value.cast_mut());
    0 // success
}

// --- НАША НОВАЯ ЗАГЛУШКА ---
fn pthread_key_delete(_env: &mut Environment, key: pthread_key_t) -> i32 {
    log!("Warning: pthread_key_delete({}) called (stubbed)", key);
    // Просто возвращаем 0 (успех), чтобы успокоить игру
    0
}

// --- ОБНОВЛЕННЫЙ СПИСОК ЭКСПОРТА ---
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(pthread_key_create(_, _)),
    export_c_func!(pthread_getspecific(_)),
    export_c_func!(pthread_setspecific(_, _)),
    export_c_func!(pthread_key_delete(_)), // Регистрируем новую функцию
];
