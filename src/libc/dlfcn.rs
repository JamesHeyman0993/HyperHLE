/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! `dlfcn.h` (`dlopen()` and friends)
//! Реализация подсистемы динамического связывания POSIX для HLE-эмуляции.
//! Код спроектирован с учетом устойчивости к некорректному доступу к памяти со
//! стороны гостевого приложения.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, MutVoidPtr, Ptr};
use crate::Environment;

/// Псевдо-дескриптор для доступа к глобальной области видимости символов (main executable).
/// В операционных системах семейства Darwin/iOS RTLD_DEFAULT традиционно равен (void*)-2.
const RTLD_DEFAULT: MutVoidPtr = Ptr::from_bits(-2 as _);

/// Проверяет, является ли запрашиваемая библиотека известной эмулятору
/// (присутствует в статическом списке DYLIB_LIST).
fn is_known_library(path: &str) -> bool {
    crate::dyld::DYLIB_LIST
        .iter()
        .any(|dylib| dylib.path == path || dylib.aliases.contains(&path))
}

/// Реализация функции `dlopen` стандарта POSIX.
/// Загружает динамическую библиотеку в адресное пространство процесса (или симулирует этот процесс в HLE).
/// Возвращает дескриптор загруженной библиотеки или NULL в случае отсутствия файла или ошибки чтения.
fn dlopen(env: &mut Environment, path: ConstPtr<u8>, _mode: i32) -> MutVoidPtr {
    if path.is_null() {
        return RTLD_DEFAULT;
    }

    // БЕЗОПАСНОСТЬ: Осуществляем защищенное чтение строки пути из гостевой памяти.
    let path_str = match env.mem.cstr_at_utf8(path) {
        Ok(s) => s,
        Err(e) => {
            log!(
                "Warning: dlopen() failed to safely read path string from guest memory: {:?}",
                e
            );
            return Ptr::null();
        }
    };

    if !is_known_library(path_str) {
        log!(
            "Warning: dlopen() returning NULL for requested but unknown library: {}",
            path_str
        );
        return Ptr::null();
    }

    path.cast_mut().cast()
}

/// Реализация функции `dlsym` стандарта POSIX.
/// Выполняет поиск адреса экспортированного символа (функции или переменной) в загруженном модуле.
fn dlsym(env: &mut Environment, handle: MutVoidPtr, symbol: ConstPtr<u8>) -> MutVoidPtr {
    // БЕЗОПАСНОСТЬ: Валидация переданного дескриптора.
    if handle != RTLD_DEFAULT {
        let handle_path_ptr: ConstPtr<u8> = handle.cast().cast_const();
        let handle_str = match env.mem.cstr_at_utf8(handle_path_ptr) {
            Ok(s) => s,
            Err(_) => {
                log!("Warning: dlsym() returning NULL due to invalid or corrupted handle pointer (possible UAF)");
                return Ptr::null();
            }
        };

        if !is_known_library(handle_str) {
            log!(
                "Warning: dlsym() returning NULL due to an unknown library handle: {}",
                handle_str
            );
            return Ptr::null();
        }
    }

    // БЕЗОПАСНОСТЬ: Защита от передачи NULL в качестве имени искомого символа.
    if symbol.is_null() {
        log!("Warning: dlsym() called with a NULL symbol pointer");
        return Ptr::null();
    }

    // БЕЗОПАСНОСТЬ: Чтение строкового имени символа из гостевой памяти.
    // ОФОРМЛЕНИЕ ИСПРАВЛЕНИЯ BORROW CHECKER: .to_owned() преобразует временный &str 
    // в независимый String, сбрасывая неизменяемое заимствование (immutable borrow) с env.mem.
    let symbol_str = match env.mem.cstr_at_utf8(symbol) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            log!("Warning: dlsym() returning NULL due to invalid symbol string pointer in guest memory");
            return Ptr::null();
        }
    };

    // В бинарном формате Mach-O (платформы Apple) C-символы компилируются с префиксом подчеркивания.
    let mut symbol_formatted = format!("_{}", symbol_str);

    // Попытка разрешить адрес через подсистему динамического загрузчика эмулятора (dyld).
    match env
        .dyld
        .create_proc_address(&mut env.mem, &mut env.cpu, &symbol_formatted)
    {
        Ok(addr) => Ptr::from_bits(addr.addr_with_thumb_bit()),
        Err(_) => {
            // --- AUTOMATED WILDCARD INTERCEPT FALLBACK ---
            // Теперь lower_sym безопасно читает из выделенной строки без конфликтов заимствования.
            let lower_sym = symbol_str.to_lowercase();
            if lower_sym.contains("iap") || 
               lower_sym.contains("kontagent") || 
               lower_sym.contains("playhaven") || 
               lower_sym.contains("flurry") ||
               lower_sym.contains("analytics") {
                log!("dlsym: Intercepted missing Unity native plugin hook '{}'. Re-routing to zero-stub safely.", symbol_str);
                
                // Перенаправляем цель вызова на нашу безопасную пустую заглушку
                symbol_formatted = "_dispatch_dummy_zero_stub".to_string();
                if let Ok(addr) = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, &symbol_formatted) {
                    return Ptr::from_bits(addr.addr_with_thumb_bit());
                }
            }

            log!(
                "Warning: dlsym() returning NULL for valid but unimplemented function {}",
                symbol_formatted
            );
            Ptr::null()
        }
    }
}

/// Реализация функции `dlclose` стандарта POSIX.
fn dlclose(env: &mut Environment, handle: MutVoidPtr) -> i32 {
    if handle == RTLD_DEFAULT {
        return 0;
    }

    let handle_path_ptr: ConstPtr<u8> = handle.cast().cast_const();

    match env.mem.cstr_at_utf8(handle_path_ptr) {
        Ok(handle_str) => {
            if !is_known_library(handle_str) {
                log!(
                    "Warning: dlclose() called on unknown or already freed library handle: {}",
                    handle_str
                );
                return -1;
            }
            0
        }
        Err(_) => {
            log!("Warning: dlclose() called with an invalid or corrupted memory pointer");
            -1
        }
    }
}

// Экспорт C-функций в глобальное адресное пространство гостевого процесса.
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dlopen(_, _)),
    export_c_func!(dlsym(_, _)),
    export_c_func!(dlclose(_)),
];
