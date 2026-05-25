/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::mem::{ConstVoidPtr, MutVoidPtr}; // Убран неиспользуемый `Ptr`
use crate::Environment;
use std::collections::HashMap;

// Хранилище обработчиков сигналов.
#[derive(Default)]
pub struct State {
    pub handlers: HashMap<i32, MutVoidPtr>,
}

// Стандартные POSIX-константы для сигналов
const SIG_DFL: u32 = 0;
// const SIG_IGN: u32 = 1;
// const SIG_ERR: u32 = 0xFFFFFFFF; // -1

fn sigaction(env: &mut Environment, signum: i32, act: ConstVoidPtr, old_act: MutVoidPtr) -> i32 {
    set_errno(env, 0);
    
    // If Mono wants to see the previous handler configuration, 
    // safely zero it out using the supported .write() API.
    if !old_act.is_null() {
        let zero_buf = [0u8; 16];
        env.mem.write(old_act.cast(), zero_buf);
    }
    
    0
}

fn signal(env: &mut Environment, signum: i32, handler: MutVoidPtr) -> MutVoidPtr {
    set_errno(env, 0);
    // Честная эмуляция: сохраняем новый обработчик и возвращаем старый.
    let old_handler = env
        .libc_state
        .signal
        .handlers
        .insert(signum, handler)
        // ИСПОЛЬЗУЕМ from_bits вместо new
        .unwrap_or_else(|| MutVoidPtr::from_bits(SIG_DFL));
    old_handler
}

fn sigprocmask(env: &mut Environment, _how: i32, _set: ConstVoidPtr, _old_set: MutVoidPtr) -> i32 {
    set_errno(env, 0);
    0
}

fn sigaltstack(env: &mut Environment, _ss: ConstVoidPtr, _old_ss: MutVoidPtr) -> i32 {
    set_errno(env, 0);
    0
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sigaction(_, _, _)),
    export_c_func!(signal(_, _)),
    export_c_func!(sigprocmask(_, _, _)), 
    export_c_func!(sigaltstack(_, _)),    
];
