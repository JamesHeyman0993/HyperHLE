/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Dynamic linker.
//!
//! iPhone OS's dynamic linker, `dyld`, is the namesake of this module.
//!
//! This is where the magic of "high-level emulation" can begin to happen.
//! The guest app will reference various functions, constants, classes etc from
//! iPhone OS's system frameworks and other dynamically-linked libraries, but
//! instead of actually loading and linking the original framework binaries,
//! this "dynamic linker" will generate appropriate stubs for calling into
//! touchHLE's own implementations of the frameworks, which are "host code"
//! (i.e. not themselves running under emulation).
//!
//! This also does normal dynamic linking for libgcc, libstdc++, etc.
//!
//! See [crate::mach_o] for resources.

mod dylib_list;

use crate::abi::{CallFromGuest, GuestFunction};
use crate::bundle;
use crate::cpu::Cpu;
use crate::frameworks::foundation::ns_string;
use crate::mach_o::{MachO, SectionType};
use crate::mem::{ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr};
use crate::objc::{nil, ClassExports, ObjC};
use crate::Environment;
use std::collections::HashMap;

pub use dylib_list::DYLIB_LIST;

/// Struct used to expose a host implementation of a dynamic library (usually a
/// framework) to the linker.
///
/// Each module that wants to expose a library to guest code should export a
/// constant using this type, which collects all the relevant [ClassExports],
/// [ConstantExports] and [FunctionExports] for the library. For example:
///
/// ```ignore
/// pub const DYLIB: HostDylib = HostDylib {
///     path: "/System/Library/Frameworks/FooBarKit.framework/FooBarKit",
///     aliases: &[],
///     class_exports: &[baz::CLASSES],
///     constant_exports: &[qux::CONSTANTS],
///     function_exports: &[qux::FUNCTIONS, baz::FUNCTIONS],
/// };
/// ```
///
/// The `path` should be the canonical notional filesystem path that the library
/// is referenced by on the real OS, for example `"/usr/lib/libobjc.A.dylib"`
/// or `"/System/Library/Frameworks/Foundation.framework/Foundation"`. For
/// libraries that have several symlinked paths, non-canonical alternate
/// paths can be listed under `aliases`, for example `"/usr/lib/libobjc.dylib"`.
pub struct HostDylib {
    pub path: &'static str,
    pub aliases: &'static [&'static str],
    pub class_exports: &'static [ClassExports],
    pub constant_exports: &'static [ConstantExports],
    pub function_exports: &'static [FunctionExports],
}

pub type HostFunction = &'static dyn CallFromGuest;

/// Type for lists of functions exported by host implementations of dynamic
/// libraries (usually frameworks).
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type, e.g.:
///
/// ```ignore
/// pub const FUNCTIONS: FunctionExports = &[
///    ("_NSFoo", &/* ... */),
///    ("_NSBar", &/* ... */),
///    /* ... */
/// ];
/// ```
///
/// All the constants like this can then be collected into a [HostDylib].
///
/// The strings are the mangled symbol names. For C functions, this is just the
/// name prefixed with an underscore.
///
/// For convenience, use [export_c_func]:
///
/// ```ignore
/// pub const FUNCTIONS: FunctionExports = &[
///     export_c_func!(NSFoo(_, _)),
///     export_c_func!(NSBar()),
/// ];
/// ```
///
/// See also [ConstantExports] and [ClassExports].
pub type FunctionExports = &'static [(&'static str, HostFunction)];

/// Macro for exporting a function with C-style name mangling. See
/// [FunctionExports].
///
/// ```ignore
/// export_c_func!(NSFoo(_, _))
/// ```
///
/// will desugar to:
///
/// ```ignore
/// ("_NSFoo", &(NSFoo as (&mut Environment, _, _) -> _))
/// ```
///
/// The function needs to be explicitly casted because a bare function reference
/// defaults to a different type than a pure fn pointer, which is the type that
/// [CallFromGuest] is implemented on. This macro will do the casting for you,
/// but you will need to supply an underscore for each parameter.
#[macro_export]
macro_rules! export_c_func {
    ($name:ident ($($_:ty),*)) => {
        (
            concat!("_", stringify!($name)),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func; // #[macro_export] is weird...

/// Other variant of [export_c_func] macro, allowing to define an alias
/// for the exporting function. This is useful then alias may contain
/// characters not normally allowed for Rust function's names. (e.g. `$`)
#[macro_export]
macro_rules! export_c_func_aliased {
    ($alias:literal, $name:ident ($($_:ty),*)) => {
        (
            concat!("_", $alias),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func_aliased; // #[macro_export] is weird...

/// Type for describing a constant (C `extern const` symbol) that will be
/// created by the linker if the guest app references it. See [ConstantExports].
pub enum HostConstant {
    NSString(&'static str),
    NullPtr,
    Custom(fn(&mut Environment) -> ConstVoidPtr),
}

/// Type for lists of constants exported by host implementations of  dynamic
/// libraries (usually frameworks).
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type, e.g.:
///
/// ```ignore
/// pub const CONSTANT: ConstantExports = &[
///    ("_kNSFooBar", HostConstant::NSString("NSFooBar")),
///    /* ... */
/// ];
/// ```
///
/// All the constants like this can then be collected into a [HostDylib].
///
/// The strings are the mangled symbol names. For C constants, this is just the
/// name prefixed with an underscore.
///
/// See also [FunctionExports], [ClassExports].
pub type ConstantExports = &'static [(&'static str, HostConstant)];

/// Search the list of [HostDylib]s for a class/constant/function by its symbol.
///
/// Example usage: `search_host_dylibs(|dylib| dylib.function_exports, "_foo")`
pub fn search_host_dylibs<T, F>(get_exports: F, symbol: &str) -> Option<&'static (&'static str, T)>
where
    F: Fn(&HostDylib) -> &'static [&'static [(&'static str, T)]],
{
    DYLIB_LIST
        .iter()
        .copied()
        .map(get_exports)
        .find_map(|lists| search_lists(lists, symbol))
}

/// Helper for working with [ClassExports]/[ConstantExports]/[FunctionExports].
fn search_lists<T>(
    lists: &'static [&'static [(&'static str, T)]],
    symbol: &str,
) -> Option<&'static (&'static str, T)> {
    lists
        .iter()
        .flat_map(|&n| n)
        .find(|&(sym, _)| *sym == symbol)
}

fn encode_a32_svc(imm: u32) -> u32 {
    assert!(imm & 0xff000000 == 0);
    imm | 0xef000000
}
fn encode_a32_ret() -> u32 {
    0xe12fff1e
}
fn encode_a32_trap() -> u32 {
    0xe7ffdefe
}

fn write_return_to_host_routine(mem: &mut Mem, svc: u32) -> GuestFunction {
    let routine = [
        encode_a32_svc(svc),
        encode_a32_trap(),
    ];
    let ptr: MutPtr<u32> = mem.alloc(4 * 2).cast();
    mem.write(ptr + 0, routine[0]);
    mem.write(ptr + 1, routine[1]);
    let ptr = GuestFunction::from_addr_with_thumb_bit(ptr.to_bits());
    assert!(!ptr.is_thumb());
    ptr
}

pub struct Dyld {
    linked_host_functions: Vec<(&'static str, HostFunction)>,
    return_to_host_routine: Option<GuestFunction>,
    thread_exit_routine: Option<GuestFunction>,
    constants_to_link_later: Vec<(MutPtr<ConstVoidPtr>, &'static HostConstant)>,
    non_lazy_host_functions: HashMap<&'static str, GuestFunction>,
}

impl Dyld {
    pub const SVC_LAZY_LINK: u32 = 0;
    pub const SVC_THREAD_EXIT: u32 = 1;
    pub const SVC_RETURN_TO_HOST: u32 = 2;
    pub const SVC_LINKED_FUNCTIONS_BASE: u32 = Self::SVC_RETURN_TO_HOST + 1;
    pub const SVC_LAZY_LINK_RET_FLAG: u32 = 0x800000;

    const SYMBOL_STUB1_INSTRUCTIONS: [u32; 1] = [0xe59ff000];
    const SYMBOL_STUB_INSTRUCTIONS: [u32; 2] = [0xe59fc000, 0xe59cf000];
    const PIC_SYMBOL_STUB_INSTRUCTIONS: [u32; 3] = [0xe59fc004, 0xe08fc00c, 0xe59cf000];

    pub fn new() -> Dyld {
        Dyld {
            linked_host_functions: Vec::new(),
            return_to_host_routine: None,
            thread_exit_routine: None,
            constants_to_link_later: Vec::new(),
            non_lazy_host_functions: HashMap::new(),
        }
    }

    pub fn return_to_host_routine(&self) -> GuestFunction {
        self.return_to_host_routine.unwrap()
    }

    pub fn thread_exit_routine(&self) -> GuestFunction {
        self.thread_exit_routine.unwrap()
    }

    pub fn do_initial_linking(
        &mut self,
        bundle: &bundle::Bundle,
        bins: &[MachO],
        mem: &mut Mem,
        objc: &mut ObjC,
    ) {
        assert!(self.return_to_host_routine.is_none());
        assert!(self.thread_exit_routine.is_none());
        self.return_to_host_routine =
            Some(write_return_to_host_routine(mem, Self::SVC_RETURN_TO_HOST));
        self.thread_exit_routine = Some(write_return_to_host_routine(mem, Self::SVC_THREAD_EXIT));

        objc.register_bin_selectors(&bins[0], mem);
        objc.register_host_selectors(mem);
        for bin in bins {
            self.setup_lazy_linking(bin, mem);
            self.do_non_lazy_linking(bin, bins, mem, objc);
        }

        objc.register_bin_classes(bundle, &bins[0], mem);
        objc.register_bin_categories(&bins[0], mem);

        ns_string::register_constant_strings(&bins[0], mem, objc);

        for dylib in bins.iter().skip(1) {
            let mut patch_count = 0;
            for sym in [
                "__ZSt19__throw_logic_errorPKc",
                "__ZSt20__throw_length_errorPKc",
                "__ZSt20__throw_out_of_rangePKc",
                "__ZSt17__throw_bad_allocv",
                "__ZSt16__throw_bad_castv",
                "__ZSt19__throw_range_errorPKc",
                "__ZSt22__throw_overflow_errorPKc",
                "__ZSt23__throw_underflow_errorPKc",
                "__ZSt21__throw_runtime_errorPKc",
                "__ZSt24__throw_invalid_argumentPKc",
                "__ZSt23__throw_ios_failurePKc",
                "__ZSt18__throw_bad_typeidv",
                "__ZSt19__throw_bad_exceptionv",
                "__ZSt25__throw_bad_function_callv",
            ] {
                let Some(&entry_with_thumb_bit) = dylib.exported_symbols.get(sym) else {
                    continue;
                };
                let entry = entry_with_thumb_bit & !1;
                let is_thumb = (entry_with_thumb_bit & 1) != 0;
                if is_thumb {
                    let function_ptr: MutPtr<u32> = Ptr::from_bits(entry);
                    mem.write(function_ptr, 0x46C0_4770);
                } else {
                    let function_ptr: MutPtr<u32> = Ptr::from_bits(entry);
                    mem.write(function_ptr, 0xE12FFF1E);
                }
                patch_count += 1;
                log_dbg!(
                    "Patched libstdc++ {} at {:#x} (thumb={}) -> bx lr",
                    sym,
                    entry_with_thumb_bit,
                    is_thumb
                );
            }
            if patch_count > 0 {
                log!(
                    "Patched {} libstdc++ std::__throw_* helpers to return \
                     instead of throwing (avoids host-process abort when \
                     guest C++ code hits soft failures like std::string(NULL)).",
                    patch_count
                );
            }
        }
    }

    pub fn dump_lazy_symbols(
        &mut self,
        bins: &[MachO],
        file: &mut std::fs::File,
    ) -> Result<(), std::io::Error> {
        use std::io::Write;
        let stubs = bins[0].get_section(SectionType::SymbolStubs).unwrap();
        let info = stubs.dyld_indirect_symbol_info.as_ref().unwrap();
        writeln!(
            file,
            "{{\n    \"object\":\"lazy_symbols\",\n    \"symbols\": ["
        )?;
        'sym: for (i, symbol) in info.indirect_undef_symbols.iter().enumerate() {
            let comma = if i == info.indirect_undef_symbols.len() - 1 {
                ""
            } else {
                ","
            };
            let symbol = symbol.as_ref().unwrap();
            if let Some(&(_, _)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
                writeln!(
                    file,
                    "        {{ \"symbol\": \"{symbol}\", \"linked_to\": \"host\"}}{comma}"
                )?;
                continue;
            }
            for dylib in bins.iter() {
                if dylib.exported_symbols.contains_key(symbol) {
                    writeln!(
                        file,
                        "        {{ \"symbol\": \"{}\", \"linked_to\": \"dylib\", \"dylib\": \"{}\"}}{}",
                        symbol, dylib.name, comma
                    )?;
                    continue 'sym;
                }
            }
            writeln!(file, "        {{ \"symbol\": \"{symbol}\" }}{comma}")?;
        }
        writeln!(file, "    ]\n}}")
    }

    pub fn dump_host_symbols(file: &mut std::fs::File) -> Result<(), std::io::Error> {
        use std::io::Write;
        for dylib in DYLIB_LIST {
            writeln!(file, "// {}", dylib.path)?;
            for alias in dylib.aliases {
                writeln!(file, "// {alias}")?;
            }
            for (class_name, _) in dylib.class_exports.iter().copied().flatten() {
                writeln!(file, "@interface {class_name}")?;
                writeln!(file, "@end")?;
                writeln!(file, "@implementation {class_name}")?;
                writeln!(file, "@end")?;
            }
            for (constant_symbol, _) in dylib.constant_exports.iter().copied().flatten() {
                writeln!(file, "int {};", constant_symbol.strip_prefix("_").unwrap())?;
            }
            for (function_symbol, _) in dylib.function_exports.iter().copied().flatten() {
                writeln!(
                    file,
                    "void {}() {{}}",
                    function_symbol.strip_prefix("_").unwrap()
                )?;
            }
        }
        Ok(())
    }

    pub fn do_initial_linking_with_no_bins(&mut self, mem: &mut Mem, objc: &mut ObjC) {
        assert!(self.return_to_host_routine.is_none());
        assert!(self.thread_exit_routine.is_none());
        self.return_to_host_routine =
            Some(write_return_to_host_routine(mem, Self::SVC_RETURN_TO_HOST));
        self.thread_exit_routine = Some(write_return_to_host_routine(mem, Self::SVC_THREAD_EXIT));

        objc.register_host_selectors(mem);
    }

    fn setup_lazy_linking(&self, bin: &MachO, mem: &mut Mem) {
        let Some(stubs) = bin.get_section(SectionType::SymbolStubs) else {
            return;
        };

        let Some(indirect_info) = stubs.dyld_indirect_symbol_info.as_ref() else {
            log!(
                "Warning: setup_lazy_linking: __symbol_stub section is missing dyld indirect symbol info; skipping lazy stub rewrite."
            );
            return;
        };
        let entry_size = indirect_info.entry_size;

        let expected_instructions: &[u32] = match entry_size {
            4 => &[],
            12 => Self::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            16 => Self::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            other => {
                log!(
                    "Warning: setup_lazy_linking: unsupported stub entry size {}; skipping lazy stub rewrite.",
                    other
                );
                return;
            }
        };

        assert!(stubs.size % entry_size == 0);
        let stub_count = stubs.size / entry_size;
        for i in 0..stub_count {
            let ptr: MutPtr<u32> = Ptr::from_bits(stubs.addr + i * entry_size);

            let mut mismatch = false;
            for (j, &instr) in expected_instructions.iter().enumerate() {
                if mem.read(ptr + j.try_into().unwrap()) != instr {
                    log_dbg!(
                        "Warning: stub {} at {:#x} has unexpected instruction at offset {} \
                         (expected {:#010x}, got {:#010x}), skipping",
                        i,
                        stubs.addr + i * entry_size,
                        j,
                        instr,
                        mem.read::<u32, true>(ptr + j.try_into().unwrap())
                    );
                    mismatch = true;
                    break;
                }
            }
            if mismatch {
                continue;
            }

            if entry_size == 4 {
                mem.write(ptr + 0, encode_a32_svc(Self::SVC_LAZY_LINK_RET_FLAG));
            } else {
                mem.write(ptr + 0, encode_a32_svc(Self::SVC_LAZY_LINK));
                mem.write(ptr + 1, encode_a32_ret());
            }
            if entry_size == 16 {
                mem.write(ptr + 2, encode_a32_trap());
            }
        }
    }

    fn do_non_lazy_linking(&mut self, bin: &MachO, bins: &[MachO], mem: &mut Mem, objc: &mut ObjC) {
        let mut unhandled_relocations: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut block_class_addrs: HashMap<String, u32> = HashMap::new();
        let mut cxxabi_vtable_addrs: HashMap<String, u32> = HashMap::new();
        for &(ptr_ptr, ref name) in &bin.external_relocations {
            let ptr_ptr: MutPtr<ConstVoidPtr> = Ptr::from_bits(ptr_ptr);
            let offset: u32 = mem.read(ptr_ptr).to_bits();
            let target: ConstVoidPtr = if let Some(name) = name.strip_prefix("_OBJC_CLASS_$_") {
                objc.link_class(name, false, mem)
                    .cast()
                    .cast_const()
            } else if let Some(name) = name.strip_prefix("_OBJC_METACLASS_$_") {
                objc.link_class(name, true, mem)
                    .cast()
                    .cast_const()
            } else if name == "___CFConstantStringClassReference" {
                nil.cast().cast_const()
            } else if name == "___mb_cur_max" {
                let val_ptr: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(val_ptr, 1u32);
                log_dbg!("Stubbed ___mb_cur_max at {:?}", val_ptr);
                val_ptr.cast().cast_const()
            } else if name == "dyld_stub_binder" || name == "_dyld_stub_binder" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                log_dbg!("Stubbed dyld_stub_binder at {:?}", fn_ptr);
                fn_ptr.cast().cast_const()
            } else if name == "__NSConcreteGlobalBlock" || name == "__NSConcreteStackBlock" {
                let addr = *block_class_addrs
                    .entry(name.clone())
                    .or_insert_with(|| mem.alloc(16).to_bits());
                log_dbg!("Patched block class descriptor {} -> {:#x}", name, addr);
                Ptr::from_bits(addr)
            } else if name == "__ZTVN10__cxxabiv117__class_type_infoE"
                || name == "__ZTVN10__cxxabiv120__si_class_type_infoE"
                || name == "__ZTVN10__cxxabiv121__vmi_class_type_infoE"
            {
                let addr = *cxxabi_vtable_addrs.entry(name.clone()).or_insert_with(|| {
                    let stub: MutPtr<u32> = mem.alloc(8).cast();
                    mem.write(stub + 0, encode_a32_ret());
                    mem.write(stub + 1, encode_a32_trap());
                    let stub_addr = stub.to_bits();

                    let v: MutPtr<u32> = mem.alloc(40).cast();
                    mem.write(v + 0, 0); 
                    mem.write(v + 1, 0); 
                    for i in 2..10 {
                        mem.write(v + i, stub_addr);
                    }
                    v.to_bits()
                });
                log_dbg!("Stubbed C++ vtable {} -> {:#x}", name, addr);
                  Ptr::from_bits(addr)
            } else if name == "___gxx_personality_sj0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                log_dbg!("Stubbed ___gxx_personality_sj0 -> {:#x}", fn_ptr.to_bits());
                fn_ptr.cast().cast_const()
            } else if name == "___objc_personality_v0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                log_dbg!("Stubbed ___objc_personality_v0 -> {:#x}", fn_ptr.to_bits());
                fn_ptr.cast().cast_const()
            } else if name == "___cxa_terminate_handler"
                || name == "___cxa_unexpected_handler"
                || name == "___cxa_new_handler"
            {
                let p: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(p, 0);
                p.cast().cast_const()
            } else if name == "__dispatch_source_type_read"
                || name == "__dispatch_source_type_write"
                || name == "__dispatch_source_type_timer"
                || name == "__dispatch_source_type_data_add"
                || name == "__dispatch_source_type_data_or"
                || name == "__dispatch_source_type_signal"
                || name == "__dispatch_source_type_proc"
                || name == "__dispatch_source_type_vnode"
                || name == "__dispatch_source_type_mach_send"
                || name == "__dispatch_source_type_mach_recv"
                || name == "__dispatch_source_type_memorypressure"
                || name == "__dispatch_queue_attr_concurrent"
                || name == "__dispatch_queue_attr_serial"
                || name == "__dispatch_data_empty"
                || name == "__dispatch_data_destructor_default"
                || name == "__dispatch_data_destructor_free"
                || name == "__dispatch_data_destructor_munmap"
                || name == "__dispatch_main_q"
                || name == "__dispatch_queue_main"
            {
                let p: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(p, 0);
                p.cast().cast_const()
            } else if name == "_in6addr_any" || name == "_in6addr_loopback" {
                let p: MutPtr<u8> = mem.alloc(16).cast();
                for i in 0..16 {
                    mem.write(p + i, 0);
                }
                if name == "_in6addr_loopback" {
                    mem.write(p + 15, 1u8);
                }
                p.cast().cast_const()
            } else if name == "_NDR_record" {
                let p: MutPtr<u8> = mem.alloc(12).cast();
                for i in 0..12 {
                    mem.write(p + i, 0);
                }
                p.cast().cast_const()
            } else if let Some(&external_addr) = bins
                .iter()
                .flat_map(|other_bin| other_bin.exported_symbols.get(name))
                .next()
            {
                Ptr::from_bits(external_addr)
            } else if let Some((symbol, _)) =
                search_host_dylibs(|dylib| dylib.function_exports, name)
            {
                let trampoline_ptr = self
                    .create_proc_address_no_inval(mem, symbol)
                    .unwrap()
                    .to_ptr();
                log_dbg!(
                    "Linked external relocation to host function {} at {:?}",
                    symbol,
                    trampoline_ptr
                );
                trampoline_ptr
            } else if let Some((_, template)) = search_host_dylibs(|dylib| dylib.constant_exports, name) {
                self.constants_to_link_later.push((ptr_ptr, template));
                continue;
            } else {
                unhandled_relocations
                    .entry(name)
                    .or_default()
                    .push(ptr_ptr.to_bits());
                continue;
            };
            mem.write(
                ptr_ptr,
                Ptr::from_bits(target.to_bits().wrapping_add(offset)),
            )
        }
        for (name, addrs) in unhandled_relocations {
            log!(
                "Warning: unhandled external relocation {:?} in {:?} at {}",
                name,
                bin.name,
                addrs
                    .into_iter()
                    .map(|addr| format!("{addr:#x}"))
                    .collect::<Vec<String>>()
                    .join(", "),
            );
        }

        let Some(ptrs) = bin.get_section(SectionType::NonLazySymbolPointers) else {
            return;
        };
        let info = ptrs.dyld_indirect_symbol_info.as_ref().unwrap();

        let entry_size = info.entry_size;
        assert!(entry_size == 4);
        assert!(ptrs.size % entry_size == 0);
        let ptr_count = ptrs.size / entry_size;
        'ptr_loop: for i in 0..ptr_count {
            let Some(symbol) = info.indirect_undef_symbols[i as usize].as_deref() else {
                continue;
            };

            let ptr_ptr: MutPtr<ConstVoidPtr> = Ptr::from_bits(ptrs.addr + i * entry_size);
            for other_bin in bins {
                if let Some(&addr) = other_bin.exported_symbols.get(symbol) {
                    mem.write(ptr_ptr, Ptr::from_bits(addr));
                    continue 'ptr_loop;
                }
            }

            if symbol == "dyld_stub_binder" || symbol == "_dyld_stub_binder" {
                let trampoline_ptr = self
                    .create_proc_address_no_inval(mem, symbol)
                    .unwrap()
                    .to_ptr();
                mem.write(ptr_ptr, trampoline_ptr);
                log_dbg!(
                    "Linked non-lazy host function {} at {:?}",
                    symbol,
                    trampoline_ptr
                );
                continue;
            }

            if let Some((symbol, _)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
                let trampoline_ptr = self
                    .create_proc_address_no_inval(mem, symbol)
                    .unwrap()
                    .to_ptr();
                mem.write(ptr_ptr, trampoline_ptr);
                log_dbg!(
                    "Linked non-lazy host function {} at {:?}",
                    symbol,
                    trampoline_ptr
                );
                log_dbg!("{:?}", self.non_lazy_host_functions);
                continue;
            }
            if let Some((_, template)) = search_host_dylibs(|dylib| dylib.constant_exports, symbol)
            {
                self.constants_to_link_later.push((ptr_ptr, template));
                continue;
            }

            if symbol == "__NSConcreteStackBlock" || symbol == "__NSConcreteGlobalBlock" {
                let dummy = mem.alloc(16);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                log_dbg!(
                    "Patched non-lazy block class {} -> {:#x}",
                    symbol,
                    dummy.to_bits()
                );
                continue;
            }

            if symbol == "_OBJC_EHTYPE_id" || symbol == "_OBJC_EHTYPE_$_NSException" {
                let dummy = mem.alloc(32);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                log_dbg!(
                    "Patched ObjC EH type descriptor {} -> {:#x}",
                    symbol,
                    dummy.to_bits()
                );
                continue;
            }

            if symbol == "___objc_personality_v0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                mem.write(ptr_ptr, fn_ptr.cast().cast_const());
                log_dbg!("Stubbed ___objc_personality_v0 -> {:#x}", fn_ptr.to_bits());
                continue;
            }

            if symbol == "___mb_cur_max" {
                let val_ptr: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(val_ptr, 1u32);
                mem.write(ptr_ptr, val_ptr.cast().cast_const());
                log_dbg!("Stubbed ___mb_cur_max -> {:#x}", val_ptr.to_bits());
                continue;
            }

            if symbol == "___gxx_personality_sj0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                mem.write(ptr_ptr, fn_ptr.cast().cast_const());
                log_dbg!("Stubbed ___gxx_personality_sj0 -> {:#x}", fn_ptr.to_bits());
                continue;
            }

            if symbol == "___cxa_terminate_handler"
                || symbol == "___cxa_unexpected_handler"
                || symbol == "___cxa_new_handler"
            {
                let p: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(p, 0);
                mem.write(ptr_ptr, p.cast().cast_const());
                continue;
            }

            if symbol == "__dispatch_source_type_read"
                || symbol == "__dispatch_source_type_write"
                || symbol == "__dispatch_source_type_timer"
                || symbol == "__dispatch_source_type_data_add"
                || symbol == "__dispatch_source_type_data_or"
                || symbol == "__dispatch_source_type_signal"
                || symbol == "__dispatch_source_type_proc"
                || symbol == "__dispatch_source_type_vnode"
                || symbol == "__dispatch_source_type_mach_send"
                || symbol == "__dispatch_source_type_mach_recv"
                || symbol == "__dispatch_source_type_memorypressure"
                || symbol == "__dispatch_queue_attr_concurrent"
                || symbol == "__dispatch_queue_attr_serial"
                || symbol == "__dispatch_data_empty"
                || symbol == "__dispatch_data_destructor_default"
                || symbol == "__dispatch_data_destructor_free"
                || symbol == "__dispatch_data_destructor_munmap"
                || symbol == "__dispatch_main_q"
                || symbol == "__dispatch_queue_main"
            {
                let p: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(p, 0);
                mem.write(ptr_ptr, p.cast().cast_const());
                log_dbg!(
                    "Stubbed libdispatch identity tag {} -> {:#x}",
                    symbol,
                    p.to_bits()
                );
                continue;
            }

            if symbol == "_in6addr_any" || symbol == "_in6addr_loopback" {
                let p: MutPtr<u8> = mem.alloc(16).cast();
                for i in 0..16 {
                    mem.write(p + i, 0);
                }
                if symbol == "_in6addr_loopback" {
                    mem.write(p + 15, 1u8);
                }
                mem.write(ptr_ptr, p.cast().cast_const());
                continue;
            }

            if symbol == "_NDR_record" {
                let p: MutPtr<u8> = mem.alloc(12).cast();
                for i in 0..12 {
                    mem.write(p + i, 0);
                }
                mem.write(ptr_ptr, p.cast().cast_const());
                continue;
            }

            log!(
                "Warning: unhandled non-lazy symbol {:?} at {:?} in \"{}\"",
                symbol,
                ptr_ptr,
                bin.name
            );
        }
    }

    pub fn do_late_linking(env: &mut Environment) {
        let to_link = std::mem::take(&mut env.dyld.constants_to_link_later);
        for (symbol_ptr_ptr, template) in to_link {
            let symbol_ptr: ConstVoidPtr = match template {
                HostConstant::NSString(static_str) => {
                    let string_ptr = ns_string::get_static_str(env, static_str);
                    let string_ptr_ptr = env.mem.alloc_and_write(string_ptr);
                    string_ptr_ptr.cast().cast_const()
                }
                HostConstant::NullPtr => {
                    let null_ptr: ConstVoidPtr = Ptr::null();
                    let null_ptr_ptr = env.mem.alloc_and_write(null_ptr);
                    null_ptr_ptr.cast().cast_const()
                }
                HostConstant::Custom(f) => f(env),
            };
            env.mem.write(symbol_ptr_ptr, symbol_ptr.cast());
        }
    }

    pub fn get_svc_handler(
        &mut self,
        bins: &[MachO],
        mem: &mut Mem,
        cpu: &mut Cpu,
        svc_pc: u32,
        svc: u32,
    ) -> Option<HostFunction> {
        match svc {
            Self::SVC_LAZY_LINK | Self::SVC_LAZY_LINK_RET_FLAG => {
                self.do_lazy_link(bins, mem, cpu, svc_pc)
            }
            Self::SVC_THREAD_EXIT | Self::SVC_RETURN_TO_HOST => {
                log!(
                    "Warning: Dyld::get_svc_handler received SVC #{} (thread exit / return to host) at {:#x}; this should be handled earlier.",
                    svc, svc_pc
                );
                None
            }
            Self::SVC_LINKED_FUNCTIONS_BASE.. => {
                let f = self.linked_host_functions.get(
                    ((svc & !Self::SVC_LAZY_LINK_RET_FLAG) - Self::SVC_LINKED_FUNCTIONS_BASE)
                        as usize,
                );
                let Some(&(symbol, f)) = f else {
                    log!(
                        "Warning: Unexpected SVC #{} at {:#x}; treating as no-op (returning to caller).",
                        svc, svc_pc
                    );
                    return None;
                };
                log_dbg!("Call to host function, already linked: {}", symbol);
                Some(f)
            }
        }
    }

    fn do_lazy_link(
        &mut self,
        bins: &[MachO],
        mem: &mut Mem,
        cpu: &mut Cpu,
        svc_pc: u32,
    ) -> Option<HostFunction> {
        fn link_by_restoring_stub(
            mem: &mut Mem,
            cpu: &mut Cpu,
            linked_function: u32,
            svc_pc: u32,
            entry_size: u32,
            pic_offset: u32,
        ) -> (MutPtr<u32>, MutPtr<u32>) {
            let original_instructions: &[u32] = match entry_size {
                4 => Dyld::SYMBOL_STUB1_INSTRUCTIONS.as_slice(),
                12 => Dyld::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                16 => Dyld::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                other => {
                    log!(
                        "Warning: link_by_restoring_stub: unsupported entry size {}; falling back to 12-byte stub.",
                        other
                    );
                    Dyld::SYMBOL_STUB_INSTRUCTIONS.as_slice()
                }
            };
            let instruction_count: GuestUSize = original_instructions.len().try_into().unwrap();

            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            if entry_size == 4 {
                mem.write(stub_function_ptr, original_instructions[0] | pic_offset)
            } else {
                for (i, &instr) in original_instructions.iter().enumerate() {
                    mem.write(stub_function_ptr + i.try_into().unwrap(), instr)
                }
            }

            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), instruction_count * 4);
            let la_symbol_ptr: MutPtr<u32> = if entry_size == 12 {
                let addr = mem.read(stub_function_ptr + instruction_count);
                Ptr::from_bits(addr)
            } else {
                if entry_size == 4 {
                    let offset = mem.read(stub_function_ptr) & 0xFFF;
                    Ptr::from_bits(stub_function_ptr.to_bits() + offset + 8)
                } else {
                    let offset = mem.read(stub_function_ptr + instruction_count);
                    Ptr::from_bits(stub_function_ptr.to_bits() + offset + 12)
                }
            };
            mem.write(la_symbol_ptr, linked_function);
            (stub_function_ptr, la_symbol_ptr)
        }

        let (stubs, pic_offset) = bins
            .iter()
            .find_map(|bin| {
                let stubs = bin.get_section(SectionType::SymbolStubs)?;
                if !(stubs.addr..(stubs.addr + stubs.size)).contains(&svc_pc) {
                    return None;
                }
                let pic_offset = bin
                    .get_section(SectionType::LazySymbolPointers)
                    .map_or(0, |lazy_ptrs| lazy_ptrs.addr - stubs.addr);
                Some((stubs, pic_offset))
            })
            .unwrap();
        let info = stubs.dyld_indirect_symbol_info.as_ref().unwrap();

        let offset = svc_pc - stubs.addr;
        assert!(offset.is_multiple_of(info.entry_size));
        let idx = (offset / info.entry_size) as usize;
        let symbol = info.indirect_undef_symbols[idx].as_deref().unwrap();

        if let Some(&addr) = self.non_lazy_host_functions.get(symbol) {
            let (stub_function_ptr, la_symbol_ptr) = link_by_restoring_stub(
                mem,
                cpu,
                addr.addr_with_thumb_bit(),
                svc_pc,
                info.entry_size,
                pic_offset,
            );
            log_dbg!(
                "Linked host function {} at {:?}/{:?} to existing stub ({:?}).",
                symbol,
                stub_function_ptr,
                la_symbol_ptr,
                addr,
            );
            return None;
                }
        for dylib in bins.iter() {
            if let Some(&addr) = dylib.exported_symbols.get(symbol) {
                let (stub_function_ptr, la_symbol_ptr) =
                    link_by_restoring_stub(mem, cpu, addr, svc_pc, info.entry_size, pic_offset);
                log_dbg!(
                    "Linked {} at {:?}/{:?} to {:#x} from {}",
                    symbol,
                    stub_function_ptr,
                    la_symbol_ptr,
                    addr,
                    dylib.name
                );
                return None;
            }
        }

        if let Some(&(symbol, f)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
            let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
            let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
            if info.entry_size == 4 {
                assert!(svc < Self::SVC_LAZY_LINK_RET_FLAG);
                svc |= Self::SVC_LAZY_LINK_RET_FLAG;
            }
            self.linked_host_functions.push((symbol, f));
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            mem.write(stub_function_ptr, encode_a32_svc(svc));
            if info.entry_size != 4 {
                assert!(mem.read(stub_function_ptr + 1) == encode_a32_ret());
            }

            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);
            log_dbg!(
                "Linked {} at {:?} to host implementation",
                symbol,
                stub_function_ptr
            );
            return Some(f);
        }

        // =========================================================================
        // HyperHLE Inline Structural Intercept Fallback Upgrades
        // =========================================================================
        
        // 1. Intercept `singleton_pool::is_from` -> Overrides default logic to return 1 instead of 0
        if symbol.starts_with("_ZN5boost14singleton_pool") && symbol.contains("is_from") {
            log!("HyperHLE: Intercepted boost::singleton_pool::is_from -> dynamically routing to return 1");
            let leaked_symbol: &'static str = Box::leak(symbol.to_string().into_boxed_str());
            let f: HostFunction = &(boost_singleton_pool_is_from_override as fn(&mut Environment) -> i32);
            let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
            let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
            if info.entry_size == 4 { svc |= Self::SVC_LAZY_LINK_RET_FLAG; }
            self.linked_host_functions.push((leaked_symbol, f));
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            mem.write(stub_function_ptr, encode_a32_svc(svc));
            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);
            return Some(f);
        }

        // 2. Intercept Constructors (Ctors) -> Retains the structural this_ptr instance inside r0 register
        if symbol == "_ZN5boost5uuids22basic_random_generatorINS_6random23mersenne_twister_engineIjLm32ELm624ELm397ELm31ELj2567483615ELm11ELj4294967295ELm7ELj2636928640ELm15ELj4022730752ELm18ELj1812433253EEEEC2Ev"
            || symbol == "_ZN5boost9gregorian4dateC2ENS0_9greg_yearENS0_10greg_monthENS0_8greg_dayE"
            || symbol == "_ZN5boost9date_time16counted_time_repINS_10posix_time33millisec_posix_time_system_configEEC2ERKNS_9gregorian4dateERKNS2_13time_durationE"
            || symbol.starts_with("_ZN3glf7TlsNodeC2")
        {
            log!("HyperHLE: Intercepted Ctor -> Preserving structure this_ptr ({})", symbol);
            let leaked_symbol: &'static str = Box::leak(symbol.to_string().into_boxed_str());
            let f: HostFunction = &(boost_and_glf_constructor_handler as fn(&mut Environment));
            let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
            let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
            if info.entry_size == 4 { svc |= Self::SVC_LAZY_LINK_RET_FLAG; }
            self.linked_host_functions.push((leaked_symbol, f));
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            mem.write(stub_function_ptr, encode_a32_svc(svc));
            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);
            return Some(f);
        }

        // =========================================================================
        // FIX: Route libc++ std::string mangled names to string.rs implementations
        // =========================================================================
        if symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC1ERKS5_"
            || symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC1ERKS5_mmRKS4_"
            || symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEED1Ev"
            || symbol == "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEaSERKS5_"
        {
            log!("HyperHLE: Intercepted libc++ std::string operation -> routing to string.rs: {}", symbol);
            
            let f: HostFunction = match symbol {
                "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEED1Ev" => {
                    &(crate::libc::string::libcxx_string_destructor as fn(&mut Environment, crate::mem::MutVoidPtr))
                }
                "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEaSERKS5_" => {
                    &(crate::libc::string::libcxx_string_assign as fn(&mut Environment, crate::mem::MutVoidPtr, crate::mem::ConstVoidPtr) -> crate::mem::MutVoidPtr)
                }
                _ => { // Both constructor variants map here
                    &(crate::libc::string::libcxx_string_init as fn(&mut Environment, crate::mem::MutVoidPtr))
                }
            };

            let leaked_symbol: &'static str = Box::leak(symbol.to_string().into_boxed_str());
            let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
            let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
            if info.entry_size == 4 { svc |= Self::SVC_LAZY_LINK_RET_FLAG; }
            self.linked_host_functions.push((leaked_symbol, f));
            let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
            mem.write(stub_function_ptr, encode_a32_svc(svc));
            cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);
            return Some(f);
        }
        
        // Fallback: Default system handler for unspecified symbols
        log!(
            "Warning: call to unimplemented function {} at {:#x}; installing return-0 stub",
            symbol,
            svc_pc
        );
        let leaked_symbol: &'static str = Box::leak(symbol.to_string().into_boxed_str());
        let f: HostFunction = &(unimplemented_function_stub as fn(&mut Environment) -> i32);
        let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
        let mut svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
        if info.entry_size == 4 {
            assert!(svc < Self::SVC_LAZY_LINK_RET_FLAG);
            svc |= Self::SVC_LAZY_LINK_RET_FLAG;
        }
        self.linked_host_functions.push((leaked_symbol, f));
        let stub_function_ptr: MutPtr<u32> = Ptr::from_bits(svc_pc);
        mem.write(stub_function_ptr, encode_a32_svc(svc));
        if info.entry_size != 4 {
            assert!(mem.read(stub_function_ptr + 1) == encode_a32_ret());
        }
        cpu.invalidate_cache_range(stub_function_ptr.to_bits(), 4);
        Some(f)
    }

    pub fn create_proc_address(
        &mut self,
        mem: &mut Mem,
        cpu: &mut Cpu,
        symbol: &str,
    ) -> Result<GuestFunction, ()> {
        let function_ptr = self.create_proc_address_no_inval(mem, symbol)?;
        cpu.invalidate_cache_range(function_ptr.addr_without_thumb_bit(), 8);
        Ok(function_ptr)
    }

    fn create_proc_address_no_inval(
        &mut self,
        mem: &mut Mem,
        symbol: &str,
    ) -> Result<GuestFunction, ()> {
        if symbol == "dyld_stub_binder" || symbol == "_dyld_stub_binder" {
            let symbol_name = "dyld_stub_binder";
            if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol_name) {
                return Ok(cached_fn);
            }

            let (_, f) = export_c_func!(dyld_stub_binder(_));
            let function_ptr = self.create_guest_function(mem, symbol_name, f);
            self.non_lazy_host_functions
                .insert(symbol_name, function_ptr);
            return Ok(function_ptr);
        }

        let &(symbol, f) = search_host_dylibs(|dylib| dylib.function_exports, symbol).ok_or(())?;
        if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol) {
            return Ok(cached_fn);
        }
        let function_ptr = self.create_guest_function(mem, symbol, f);
        self.non_lazy_host_functions.insert(symbol, function_ptr);
        Ok(function_ptr)
    }

    pub fn create_guest_function(
        &mut self,
        mem: &mut Mem,
        symbol: &'static str,
        f: HostFunction,
    ) -> GuestFunction {
        let idx: u32 = self.linked_host_functions.len().try_into().unwrap();
        let svc = idx + Self::SVC_LINKED_FUNCTIONS_BASE;
        self.linked_host_functions.push((symbol, f));

        let function_ptr = mem.alloc(8);
        let function_ptr: MutPtr<u32> = function_ptr.cast();
        mem.write(function_ptr + 0, encode_a32_svc(svc));
        mem.write(function_ptr + 1, encode_a32_ret());
        GuestFunction::from_addr_with_thumb_bit(function_ptr.to_bits())
    }
}

fn dyld_stub_binder(_env: &mut Environment, _arg: u32) {
    log!(
        "Warning: dyld_stub_binder was called! Under HLE all lazy symbols are bound eagerly, so this is unexpected. Continuing as a no-op."
    );
}

fn unimplemented_function_stub(_env: &mut Environment) -> i32 {
    0
}

// =========================================================================
// HyperHLE Target Host Implementations for Intercepted Symbols
// =========================================================================

/// Special override logic targeting Boost singleton structures requiring initialization sanity checks
fn boost_singleton_pool_is_from_override(_env: &mut Environment) -> i32 {
    1
}

/// Generic handler for constructors: C++ constructors pass the `this` pointer in register R0
/// and expect it to be returned in register R0. Leaving the CPU registers completely untouched
/// safely fulfills this requirement.
fn boost_and_glf_constructor_handler(_env: &mut Environment) {
    // Intentionally leaves cpu registers untouched to allow R0 to flow back out safely
}
