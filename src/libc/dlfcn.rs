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

/// Псевдо-дескриптор для доступа к глобальной области видимости символов (main
//executable).
/// В операционных системах семейства Darwin/iOS RTLD_DEFAULT традиционно равен
//(void*)-2.
const RTLD_DEFAULT: MutVoidPtr = Ptr::from_bits(-2 as _);
/// A generic safe stub used to satisfy dlsym lookups for missing third-party plugins.
fn unity_plugin_generic_stub(_env: &mut Environment) {
    // We intentionally do nothing here except prevent a guest crash.
    log_dbg!("Unity third-party plugin stub was executed safely.");
}


/// Проверяет, является ли запрашиваемая библиотека известной эмулятору
//(присутствует в статическом списке DYLIB_LIST).
fn is_known_library(path: &str) -> bool {
    crate::dyld::DYLIB_LIST
        .iter()
        .any(|dylib| dylib.path == path || dylib.aliases.contains(&path))
}

/// Реализация функции `dlopen` стандарта POSIX.
/// Загружает динамическую библиотеку в адресное пространство процесса (или
//симулирует этот процесс в HLE).
/// Возвращает дескриптор загруженной библиотеки или NULL в случае отсутствия
//файла или ошибки чтения.
fn dlopen(env: &mut Environment, path: ConstPtr<u8>, _mode: i32) -> MutVoidPtr {
    // В соответствии со стандартом POSIX, вызов dlopen(NULL) возвращает
    // дескриптор главной программы.
    // Эмулятор предоставляет доступ к глобальным символам через специальный
    // дескриптор RTLD_DEFAULT.
    if path.is_null() {
        return RTLD_DEFAULT;
    }

    // БЕЗОПАСНОСТЬ: Осуществляем защищенное чтение строки пути из
    // неконтролируемой гостевой памяти.
    // Если указатель недействителен (Out-Of-Bounds) или строка не является
    // корректной UTF-8 последовательностью,
    // мы прерываем операцию загрузки и возвращаем NULL, не допуская паники
    // эмулятора (Denial of Service).
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

    // Если библиотека не известна системе эмуляции (например,
    // кросс-платформенный фреймворк пытается
    // загрузить специфичный для другой платформы плагин), мы мягко отклоняем
    // запрос, возвращая NULL.
    // Данное поведение ожидается гостевым приложением для "мягкой деградации"
    // (graceful degradation).
    if !is_known_library(path_str) {
        log!(
            "Warning: dlopen() returning NULL for requested but unknown library: {}",
            path_str
        );
        return Ptr::null();
    }

    // Временная архитектура: использование указателя на строку пути в памяти
    // гостя как непрозрачного дескриптора.
    // TODO: Разработать защищенную систему управления дескрипторами (Handle
    // Allocator Table) на стороне хоста,
    // чтобы предотвратить уязвимости Use-After-Free, когда приложение
    // освобождает строку пути после вызова dlopen.
    path.cast_mut().cast()
}

/// Реализация функции `dlsym` стандарта POSIX.
/// Выполняет поиск адреса экспортированного символа (функции или переменной) в
//загруженном модуле.
fn dlsym(env: &mut Environment, handle: MutVoidPtr, symbol: ConstPtr<u8>) -> MutVoidPtr {
    // ... Keep your existing safety validation checks for handle and symbol ...
    if handle != RTLD_DEFAULT { /* ... existing handle code ... */ }
    if symbol.is_null() { /* ... existing check ... */ }

    // Read the symbol string from guest memory
    let symbol_str = match env.mem.cstr_at_utf8(symbol) {
        Ok(s) => s,
        Err(_) => {
            log!("Warning: dlsym() returning NULL due to invalid symbol string pointer in guest memory");
            return Ptr::null();
        }
    };

    // --- NEW INTERCEPT ZONE FOR GHOST TOASTERS ---
    // If Unity requests these specific third-party functions, bypass dyld 
    // and manufacture a valid procedure address pointing to our safe stub.
    match symbol_str {
        "_IAPLoadProducts" | "IAPLoadProducts" |
        "_kontagentApplicationAdded" | "kontagentApplicationAdded" |
        "_kontagentStartSessionNew" | "kontagentStartSessionNew" => {
            log!("dlsym: Intercepted missing plugin symbol '{}'. Redirecting to safe stub.", symbol_str);
            
            // Generate a callable guest-space wrapper address for our host-side function
            match env.dyld.create_proc_address_from_host_fn(&mut env.mem, &mut env.cpu, unity_plugin_generic_stub) {
                Ok(addr) => return Ptr::from_bits(addr.addr_with_thumb_bit()),
                Err(_) => {
                    log!("Warning: Failed to create proc address for plugin stub.");
                    return Ptr::null();
                }
            }
        }
        _ => {} // Not a target plugin, fall through to regular processing
    }
    // --- END OF INTERCEPT ZONE ---

    // Proceed with regular Mach-O underscore lookup formatting
    let symbol_formatted = format!("_{}", symbol_str);

    match env
        .dyld
        .create_proc_address(&mut env.mem, &mut env.cpu, &symbol_formatted)
    {
        Ok(addr) => Ptr::from_bits(addr.addr_with_thumb_bit()),
        Err(_) => {
            log!(
                "Warning: dlsym() returning NULL for valid but unimplemented function {}",
                symbol_formatted
            );
            Ptr::null()
        }
    }
}

/// Реализация функции `dlclose` стандарта POSIX.
/// В HLE архитектуре выступает в роли заглушки, но строго соблюдает семантику
//возврата кодов ошибок.
fn dlclose(env: &mut Environment, handle: MutVoidPtr) -> i32 {
    if handle == RTLD_DEFAULT {
        return 0; // Операция успешна
    }

    let handle_path_ptr: ConstPtr<u8> = handle.cast().cast_const();

    // БЕЗОПАСНОСТЬ: Проверяем валидность переданного дескриптора перед
    // возвратом кода статуса.
    match env.mem.cstr_at_utf8(handle_path_ptr) {
        Ok(handle_str) => {
            if !is_known_library(handle_str) {
                log!(
                    "Warning: dlclose() called on unknown or already freed library handle: {}",
                    handle_str
                );
                return -1; // -1 стандартный код ошибки POSIX для dlclose
            }
            0 // Успех
        }
        Err(_) => {
            log!("Warning: dlclose() called with an invalid or corrupted memory pointer");
            -1 // Ошибка доступа к памяти
        }
    }
}

// Экспорт C-функций в глобальное адресное пространство гостевого процесса.
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dlopen(_, _)),
    export_c_func!(dlsym(_, _)),
    export_c_func!(dlclose(_)),
];
