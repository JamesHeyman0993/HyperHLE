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
pub type FunctionExports = &'static [(&'static str, HostFunction)];

/// Macro for exporting a function with C-style name mangling.
#[macro_export]
macro_rules! export_c_func {
    ($name:ident ($($_:ty),*)) => {
        (
            concat!("_", stringify!($name)),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func;

/// Other variant of [export_c_func] macro, allowing to define an alias
#[macro_export]
macro_rules! export_c_func_aliased {
    ($alias:literal, $name:ident ($($_:ty),*)) => {
        (
            concat!("_", $alias),
            &($name as fn(&mut $crate::Environment, $($_),*) -> _)
        )
    };
}
pub use crate::export_c_func_aliased;

/// Type for describing a constant (C `extern const` symbol) that will be
/// created by the linker if the guest app references it.
pub enum HostConstant {
    NSString(&'static str),
    NullPtr,
    Custom(fn(&mut Environment) -> ConstVoidPtr),
}

/// Type for lists of constants exported by host implementations of dynamic
/// libraries (usually frameworks).
pub type ConstantExports = &'static [(&'static str, HostConstant)];

/// Search the list of [HostDylib]s for a class/constant/function by its symbol.
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
                    "Patched {} libstdc++ std::__throw_* helpers to return instead of throwing.",
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
            let comma = if i == info.indirect_undef_symbols.len() - 1 { "" } else { "," };
            let symbol = symbol.as_ref().unwrap();
            if let Some(&(_, _)) = search_host_dylibs(|dylib| dylib.function_exports, symbol) {
                writeln!(file, "        {{ \"symbol\": \"{symbol}\", \"linked_to\": \"host\"}}{comma}")?;
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
                writeln!(file, "@interface {class_name}\n@end\n@implementation {class_name}\n@end")?;
            }
            for (constant_symbol, _) in dylib.constant_exports.iter().copied().flatten() {
                writeln!(file, "int {};", constant_symbol.strip_prefix("_").unwrap())?;
            }
            for (function_symbol, _) in dylib.function_exports.iter().copied().flatten() {
                writeln!(file, "void {}() {{}}", function_symbol.strip_prefix("_").unwrap())?;
            }
        }
        Ok(())
    }

    pub fn do_initial_linking_with_no_bins(&mut self, mem: &mut Mem, objc: &mut ObjC) {
        assert!(self.return_to_host_routine.is_none());
        assert!(self.thread_exit_routine.is_none());
        self.return_to_host_routine = Some(write_return_to_host_routine(mem, Self::SVC_RETURN_TO_HOST));
        self.thread_exit_routine = Some(write_return_to_host_routine(mem, Self::SVC_THREAD_EXIT));
        objc.register_host_selectors(mem);
    }

    fn setup_lazy_linking(&self, bin: &MachO, mem: &mut Mem) {
        let Some(stubs) = bin.get_section(SectionType::SymbolStubs) else {
            return;
        };

        let entry_size = stubs.dyld_indirect_symbol_info.as_ref().unwrap().entry_size;
        let expected_instructions = match entry_size {
            4 => &[],
            12 => Self::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            16 => Self::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
            _ => unimplemented!(),
        };

        assert!(stubs.size % entry_size == 0);
        let stub_count = stubs.size / entry_size;
        for i in 0..stub_count {
            let ptr: MutPtr<u32> = Ptr::from_bits(stubs.addr + i * entry_size);
            let mut mismatch = false;
            for (j, &instr) in expected_instructions.iter().enumerate() {
                if mem.read(ptr + j.try_into().unwrap()) != instr {
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
                objc.link_class(name, false, mem).cast().cast_const()
            } else if let Some(name) = name.strip_prefix("_OBJC_METACLASS_$_") {
                objc.link_class(name, true, mem).cast().cast_const()
            } else if name == "___CFConstantStringClassReference" {
                nil.cast().cast_const()
            } else if name == "___mb_cur_max" {
                let val_ptr: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(val_ptr, 1u32);
                val_ptr.cast().cast_const()
            } else if name == "dyld_stub_binder" || name == "_dyld_stub_binder" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                fn_ptr.cast().cast_const()
            } else if name == "___dynamic_cast" {
                let trampoline_ptr = self.create_proc_address_no_inval(mem, "___dynamic_cast").unwrap().to_ptr();
                trampoline_ptr
            } else if name == "_sqlite3_prepare_v2"
                || name == "_sqlite3_step"
                || name == "_sqlite3_errmsg"
                || name == "__dyld_get_image_header"
                || name == "__dyld_register_func_for_add_image"
                || name == "__dyld_register_func_for_remove_image"
                || name == "_CrittercismJKParseUTF8String"
            {
                let trampoline_ptr = self.create_proc_address_no_inval(mem, name).unwrap().to_ptr();
                trampoline_ptr
            } else if name == "__NSConcreteGlobalBlock" || name == "__NSConcreteStackBlock" {
                let addr = *block_class_addrs.entry(name.clone()).or_insert_with(|| mem.alloc(16).to_bits());
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
                    for i in 2..10 { mem.write(v + i, stub_addr); }
                    v.to_bits()
                });
                Ptr::from_bits(addr)
                } else if name == "___gxx_personality_sj0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                fn_ptr.cast().cast_const()
            } else if name == "___cxa_terminate_handler"
                || name == "___cxa_unexpected_handler"
                || name == "___cxa_new_handler"
            {
                let p: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(p, 0);
                p.cast().cast_const()
            } else if let Some(&external_addr) = bins.iter().flat_map(|o| o.exported_symbols.get(name)).next() {
                Ptr::from_bits(external_addr)
            } else if let Some((symbol, _)) = search_host_dylibs(|d| d.function_exports, name) {
                let trampoline_ptr = self.create_proc_address_no_inval(mem, symbol).unwrap().to_ptr();
                trampoline_ptr
            } else if search_host_dylibs(|d| d.constant_exports, name).is_some() {
                continue;
            } else {
                unhandled_relocations.entry(name).or_default().push(ptr_ptr.to_bits());
                continue;
            };
            mem.write(ptr_ptr, Ptr::from_bits(target.to_bits().wrapping_add(offset)))
        }

        for (name, addrs) in unhandled_relocations {
            log!(
                "Warning: unhandled external relocation {:?} in {:?} at {}",
                name, bin.name,
                addrs.into_iter().map(|a| format!("{a:#x}")).collect::<Vec<String>>().join(", "),
            );
        }

        let Some(ptrs) = bin.get_section(SectionType::NonLazySymbolPointers) else { return; };
        let info = ptrs.dyld_indirect_symbol_info.as_ref().unwrap();

        let entry_size = info.entry_size;
        assert!(entry_size == 4);
        assert!(ptrs.size % entry_size == 0);
        let ptr_count = ptrs.size / entry_size;
        'ptr_loop: for i in 0..ptr_count {
            let Some(symbol) = info.indirect_undef_symbols[i as usize].as_deref() else { continue; };
            let ptr_ptr: MutPtr<ConstVoidPtr> = Ptr::from_bits(ptrs.addr + i * entry_size);
            
            for other_bin in bins {
                if let Some(&addr) = other_bin.exported_symbols.get(symbol) {
                    mem.write(ptr_ptr, Ptr::from_bits(addr));
                    continue 'ptr_loop;
                }
            }

            if symbol == "dyld_stub_binder" || symbol == "_dyld_stub_binder" {
                let trampoline_ptr = self.create_proc_address_no_inval(mem, symbol).unwrap().to_ptr();
                mem.write(ptr_ptr, trampoline_ptr);
                continue;
            }

            if let Some((symbol, _)) = search_host_dylibs(|d| d.function_exports, symbol) {
                let trampoline_ptr = self.create_proc_address_no_inval(mem, symbol).unwrap().to_ptr();
                mem.write(ptr_ptr, trampoline_ptr);
                continue;
            }
            if let Some((_, template)) = search_host_dylibs(|d| d.constant_exports, symbol) {
                self.constants_to_link_later.push((ptr_ptr, template));
                continue;
            }

            if symbol == "__NSConcreteStackBlock" || symbol == "__NSConcreteGlobalBlock" {
                let dummy = mem.alloc(16);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                continue;
            }
            if symbol == "_OBJC_EHTYPE_id" || symbol == "_OBJC_EHTYPE_$_NSException" {
                let dummy = mem.alloc(32);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                continue;
            }
            
            if symbol == "_NSMetadataItemFSNameKey" 
                || symbol == "_NSMetadataItemURLKey" 
                || symbol == "_NSMetadataQueryDidFinishGatheringNotification" 
                || symbol == "_NSMetadataQueryUbiquitousDocumentsScope" 
            {
 let dummy = mem.alloc(16);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                continue;
            }
            if symbol == "_in6addr_any" 
                || symbol == "_in6addr_loopback"
                || symbol == "_kCFStreamErrorDomainNetDB" 
                || symbol == "_NSURLErrorFailingURLStringErrorKey"
                || symbol == "_MPMovieDurationAvailableNotification"
                || symbol == "_UIApplicationDidChangeStatusBarFrameNotification"
                || symbol == "_NSUbiquitousKeyValueStoreDidChangeExternallyNotification"
            {
                let dummy = mem.alloc(28);
                mem.write(ptr_ptr, dummy.cast().cast_const());
                continue;
            }
            
            if symbol == "___objc_personality_v0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                mem.write(ptr_ptr, fn_ptr.cast().cast_const());
                continue;
            }

            if symbol == "___mb_cur_max" {
                let val_ptr: MutPtr<u32> = mem.alloc(4).cast();
                mem.write(val_ptr, 1u32);
                mem.write(ptr_ptr, val_ptr.cast().cast_const());
                continue;
            }

            if symbol == "___gxx_personality_sj0" {
                let fn_ptr: MutPtr<u32> = mem.alloc(8).cast();
                mem.write(fn_ptr + 0, encode_a32_ret());
                mem.write(fn_ptr + 1, encode_a32_trap());
                mem.write(ptr_ptr, fn_ptr.cast().cast_const());
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

            log!("Warning: unhandled non-lazy symbol {:?} at {:?} in \"{}\"", symbol, ptr_ptr, bin.name);
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
            Self::SVC_THREAD_EXIT | Self::SVC_RETURN_TO_HOST => unreachable!(),
            Self::SVC_LINKED_FUNCTIONS_BASE.. => {
                let f = self.linked_host_functions.get(
                    ((svc & !Self::SVC_LAZY_LINK_RET_FLAG) - Self::SVC_LINKED_FUNCTIONS_BASE) as usize,
                );
                let Some(&(symbol, f)) = f else {
                    panic!("Unexpected SVC #{svc} at {svc_pc:#x}");
                };
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
            let original_instructions = match entry_size {
                4 => Dyld::SYMBOL_STUB1_INSTRUCTIONS.as_slice(),
                12 => Dyld::SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                16 => Dyld::PIC_SYMBOL_STUB_INSTRUCTIONS.as_slice(),
                _ => unreachable!(),
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
                if !(stubs.addr..(stubs.addr + stubs.size)).contains(&svc_pc) { return None; }
                let pic_offset = bin.get_section(SectionType::LazySymbolPointers).map_or(0, |l| l.addr - stubs.addr);
                Some((stubs, pic_offset))
            })
            .unwrap();
        let info = stubs.dyld_indirect_symbol_info.as_ref().unwrap();

        let offset = svc_pc - stubs.addr;
        assert!(offset.is_multiple_of(info.entry_size));
        let idx = (offset / info.entry_size) as usize;
        let symbol = info.indirect_undef_symbols[idx].as_deref().unwrap();
        
        if symbol == "___dynamic_cast" {
            let addr = self.create_proc_address_no_inval(mem, "___dynamic_cast").unwrap();
            let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
            return None;
        }
        
                // --- FIXED CRITICAL MATH INTERCEPT ---
        let vector_math_mangled = "__ZNSt6vectorIPN5Maths10cMatrix4x4ESaIS2_EE13_M_insert_auxEN9__gnu_cxx17__normal_iteratorIPS2_S4_EERKS2_";
        if symbol == vector_math_mangled {
             log!("HyperHLE: Catching unmapped vector routine: {}. Allocating host adapter bridge.", symbol);
             
             // Instead of an infinite loop return-None loop, intercept it and build a clean host-side execution binding.
             let addr = self.create_proc_address_no_inval(mem, vector_math_mangled).unwrap();
             let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
             
             // Return the allocated host procedure runner so emulation handles the tick instantly
             let idx_f: u32 = (self.linked_host_functions.len() - 1).try_into().unwrap();
             return Some(self.linked_host_functions[idx_f as usize].1);
        }

                // --- NEW: CRYPTO SHA256 PASSTHROUGH INTERCEPT ---
        if symbol == "_CC_SHA256" || symbol == "CC_SHA256" {
             log!("HyperHLE: Catching static crypt engine link: {}. Setting pass-through safety buffer alignment.", symbol);
             let addr = self.create_proc_address_no_inval(mem, symbol).unwrap();
             let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
             let idx_f: u32 = (self.linked_host_functions.len() - 1).try_into().unwrap();
             return Some(self.linked_host_functions[idx_f as usize].1);
        }

                // --- BYPASS HACK FOR MARMALADE SDK & MEDIAPLAYER CONSTANTS ---
        if symbol == "_kUTTypeBMP" 
           || symbol == "_kCAGravityTopLeft" 
           || symbol == "_UIPasteboardTypeListString"
           || symbol.starts_with("_kAB")
           || symbol.starts_with("kAB")
           || symbol.starts_with("_MPMedia")
           || symbol.starts_with("MPMedia")
        {
             log!("HyperHLE: Catching lazy link for dependency symbol: {}", symbol);
             let addr = self.create_proc_address_no_inval(mem, symbol).unwrap();
             let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
             
             // Check if we can safely pull a valid host execution index
             if self.linked_host_functions.is_empty() {
                 let leaked_symbol: &'static str = Box::leak(symbol.to_string().into_boxed_str());
                 let stub_f: HostFunction = &(unimplemented_function_stub as fn(&mut Environment) -> i32);
                 self.linked_host_functions.push((leaked_symbol, stub_f));
             }
             let idx_f: u32 = (self.linked_host_functions.len() - 1).try_into().unwrap();
             return Some(self.linked_host_functions[idx_f as usize].1);
        }
        
        // --- NEW: CRITTERCISM PARSER INTERCEPT ---
        
        if symbol == "_CrittercismJKParseUTF8String" || symbol == "CrittercismJKParseUTF8String" {
             log!("HyperHLE: Catching lazy link for Crittercism JSON Parser. Bypassing crash engine.");
             let addr = self.create_proc_address_no_inval(mem, symbol).unwrap();
             let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
             let idx_f: u32 = (self.linked_host_functions.len() - 1).try_into().unwrap();
             return Some(self.linked_host_functions[idx_f as usize].1);
        }
        
        if let Some(&addr) = self.non_lazy_host_functions.get(symbol) {
            let _ = link_by_restoring_stub(mem, cpu, addr.addr_with_thumb_bit(), svc_pc, info.entry_size, pic_offset);
            return None;
        }

        for dylib in bins.iter() {
            if let Some(&addr) = dylib.exported_symbols.get(symbol) {
                let _ = link_by_restoring_stub(mem, cpu, addr, svc_pc, info.entry_size, pic_offset);
                return None;
            }
        }

        if let Some(&(symbol, f)) = search_host_dylibs(|d| d.function_exports, symbol) {
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
            return Some(f);
        }

        for dylib in bins.iter() {
            if let Some(&addr) = dylib.exported_symbols.get(symbol) {
                let _ = link_by_restoring_stub(mem, cpu, addr, svc_pc, info.entry_size, pic_offset);
                return None;
            }
        }
      log!("Warning: call to unimplemented function {}; installing return-0 stub", symbol);
        
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
                // --- BYPASS HACK FOR MARMALADE SDK & MEDIAPLAYER CONSTANTS ---
        if symbol == "_kUTTypeBMP" 
           || symbol == "_kCAGravityTopLeft" 
           || symbol == "_UIPasteboardTypeListString"
           || symbol.starts_with("_kAB")
           || symbol.starts_with("kAB")
           || symbol.starts_with("_MPMedia")
           || symbol.starts_with("MPMedia")
        {
            log!("HyperHLE: Intercepting and patching missing dependency symbol: {}", symbol);
            
            // Allocate an actual structural memory address alignment payload block 
            // instead of a bare function pointer. This satisfies structural data reads.
            let dummy_data_block = mem.alloc(16);
            let dummy_func = GuestFunction::from_addr_with_thumb_bit(dummy_data_block.to_bits());
            return Ok(dummy_func);
        }
                
        if symbol == "_CC_SHA256" || symbol == "CC_SHA256" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol) { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_cc_sha256_stub as fn(&mut Environment, u32, u32, u32) -> u32);
            let function_ptr = self.create_guest_function(mem, "CC_SHA256", f);
            self.non_lazy_host_functions.insert("CC_SHA256", function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "_CrittercismJKParseUTF8String" || symbol == "CrittercismJKParseUTF8String" {
            let symbol_name = "_CrittercismJKParseUTF8String";
            if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol_name) { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_crittercism_json_stub as fn(&mut Environment, u32, u32, u32, u32) -> u32);
            let function_ptr = self.create_guest_function(mem, symbol_name, f);
            self.non_lazy_host_functions.insert(symbol_name, function_ptr);
            return Ok(function_ptr);
        }

        let vector_math_mangled = "__ZNSt6vectorIPN5Maths10cMatrix4x4ESaIS2_EE13_M_insert_auxEN9__gnu_cxx17__normal_iteratorIPS2_S4_EERKS2_";
        if symbol == vector_math_mangled {
            
            if let Some(&cached_fn) = self.non_lazy_host_functions.get(vector_math_mangled) {
                return Ok(cached_fn);
            }
            let f: HostFunction = &(touchhle_vector_matrix_insert_aux as fn(&mut Environment, u32, u32, u32));
            let function_ptr = self.create_guest_function(mem, vector_math_mangled, f);
            self.non_lazy_host_functions.insert(vector_math_mangled, function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "___dynamic_cast" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("___dynamic_cast") { return Ok(cached_fn); }
            let f: HostFunction = &(touchHLE_dynamic_cast as fn(&mut Environment, u32, u32, u32, i32) -> u32);
            let function_ptr = self.create_guest_function(mem, "___dynamic_cast", f);
            self.non_lazy_host_functions.insert("___dynamic_cast", function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "_sqlite3_prepare_v2" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("_sqlite3_prepare_v2") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_sqlite3_prepare_v2 as fn(&mut Environment, u32, u32, i32, u32, u32) -> i32);
            let function_ptr = self.create_guest_function(mem, "_sqlite3_prepare_v2", f);
            self.non_lazy_host_functions.insert("_sqlite3_prepare_v2", function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "_sqlite3_step" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("_sqlite3_step") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_sqlite3_step as fn(&mut Environment, u32) -> i32);
            let function_ptr = self.create_guest_function(mem, "_sqlite3_step", f);
            self.non_lazy_host_functions.insert("_sqlite3_step", function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "_sqlite3_errmsg" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("_sqlite3_errmsg") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_sqlite3_errmsg as fn(&mut Environment, u32) -> u32);
            let function_ptr = self.create_guest_function(mem, "_sqlite3_errmsg", f);
            self.non_lazy_host_functions.insert("_sqlite3_errmsg", function_ptr);
            return Ok(function_ptr);
        }

        if symbol == "__dyld_get_image_header" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("__dyld_get_image_header") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_dyld_get_image_header as fn(&mut Environment, u32) -> u32);
            let function_ptr = self.create_guest_function(mem, "__dyld_get_image_header", f);
            self.non_lazy_host_functions.insert("__dyld_get_image_header", function_ptr);
            return Ok(function_ptr);
        }
        if symbol == "__dyld_register_func_for_add_image" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("__dyld_register_func_for_add_image") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_dyld_register_func_for_add_image as fn(&mut Environment, u32));
            let function_ptr = self.create_guest_function(mem, "__dyld_register_func_for_add_image", f);
            self.non_lazy_host_functions.insert("__dyld_register_func_for_add_image", function_ptr);
            return Ok(function_ptr);
        }
        
        if symbol == "__dyld_register_func_for_remove_image" {
            if let Some(&cached_fn) = self.non_lazy_host_functions.get("__dyld_register_func_for_remove_image") { return Ok(cached_fn); }
            let f: HostFunction = &(touchhle_dyld_register_func_for_remove_image as fn(&mut Environment, u32));
            let function_ptr = self.create_guest_function(mem, "__dyld_register_func_for_remove_image", f);
            self.non_lazy_host_functions.insert("__dyld_register_func_for_remove_image", function_ptr);
            return Ok(function_ptr);
        }
        
        if symbol == "dyld_stub_binder" || symbol == "_dyld_stub_binder" {
            let symbol_name = "dyld_stub_binder";
            if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol_name) { return Ok(cached_fn); }
            let (_, f) = export_c_func!(dyld_stub_binder(_));
            let function_ptr = self.create_guest_function(mem, symbol_name, f);
            self.non_lazy_host_functions.insert(symbol_name, function_ptr);
            return Ok(function_ptr);
        }
        let &(symbol, f) = search_host_dylibs(|d| d.function_exports, symbol).ok_or(())?;
        if let Some(&cached_fn) = self.non_lazy_host_functions.get(symbol) { return Ok(cached_fn); }
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
    panic!("dyld_stub_binder was called!");
}

fn touchHLE_dynamic_cast(_env: &mut Environment, sub_ptr: u32, _src_type: u32, _dst_type: u32, _offset_hint: i32) -> u32 {
    if sub_ptr == 0 { return 0; }
    sub_ptr
}

fn unimplemented_function_stub(_env: &mut Environment) -> i32 {
    0
}

// --- SAFE VECTOR AUX BRIDGE PATCH FOR THE MATRIX CREATION ---
/// Emulates std::vector::insert auxiliary growth logic so that structural pointers 
/// don't fall back to a return-0 corrupt state.
fn touchhle_vector_matrix_insert_aux(env: &mut Environment, vector_this: u32, position_iterator: u32, matrix_ptr_val: u32) {
    let vec_ptr: MutPtr<u32> = Ptr::from_bits(vector_this);
    
    // Read the vector internal structure bounds pointers:
    // +0: M_start (pointer to array begin)
    // +4: M_finish (pointer to current end element)
    // +8: M_end_of_storage (allocated memory ceiling)
    let start: u32 = env.mem.read(vec_ptr + 0);
    let mut finish: u32 = env.mem.read(vec_ptr + 1);
    let end_of_storage: u32 = env.mem.read(vec_ptr + 2);

    if finish < end_of_storage {
        // If there is existing padding headroom, push the memory block over by 4 bytes (1 pointer size)
        let mut current = finish;
        while current > position_iterator {
            let prev_val: u32 = env.mem.read(Ptr::<u32, false>::from_bits(current - 4));
            env.mem.write(Ptr::from_bits(current), prev_val);
            current -= 4;
        }
        // Safely write the matrix pointer element value directly into the slot position
        env.mem.write(Ptr::from_bits(position_iterator), matrix_ptr_val);
        finish += 4;
        env.mem.write(vec_ptr + 1, finish);
    } else {
        // Handle array reallocation if storage is maxed out
        let current_size = finish.saturating_sub(start);
        let new_capacity = if current_size == 0 { 4 } else { current_size * 2 };
        
        let new_start_ptr = (*env.mem).alloc(new_capacity);
        let new_start = new_start_ptr.to_bits();
        
        // Loop copy for prefix elements (before insertion point)
        let mut offset = 0;
        while start + offset < position_iterator {
            let val: u32 = env.mem.read(Ptr::<u32, false>::from_bits(start + offset));
            env.mem.write(Ptr::from_bits(new_start + offset), val);
            offset += 4;
        }
            
        // Write the newly inserted matrix pointer element
        env.mem.write(Ptr::from_bits(new_start + offset), matrix_ptr_val);
        
        // Loop copy for suffix elements (after insertion point)
        let mut suffix_offset = 0;
        while position_iterator + suffix_offset < finish {
            let val: u32 = env.mem.read(Ptr::<u32, false>::from_bits(position_iterator + suffix_offset));
            env.mem.write(Ptr::from_bits(new_start + offset + 4 + suffix_offset), val);
            suffix_offset += 4;
        }
            
        (*env.mem).write(vec_ptr + 0, new_start);
        (*env.mem).write(vec_ptr + 1, new_start + current_size + 4);
        (*env.mem).write(vec_ptr + 2, new_start + new_capacity);
    }
}
  
fn touchhle_sqlite3_prepare_v2(_env: &mut Environment, _db: u32, _z_sql: u32, _n_byte: i32, pp_stmt: u32, _pz_tail: u32) -> i32 {
    0 
}

fn touchhle_sqlite3_step(_env: &mut Environment, _stmt: u32) -> i32 {
    101 
}

fn touchhle_sqlite3_errmsg(_env: &mut Environment, _db: u32) -> u32 {
    0 
}

fn touchhle_dyld_get_image_header(_env: &mut Environment, _image_index: u32) -> u32 {
    0 
}

fn touchhle_dyld_register_func_for_add_image(_env: &mut Environment, _func: u32) {
    log_dbg!("HyperHLE: Stubbed __dyld_register_func_for_add_image");
}

fn touchhle_dyld_register_func_for_remove_image(_env: &mut Environment, _func: u32) {
    log_dbg!("HyperHLE: Stubbed __dyld_register_func_for_remove_image");
}

fn touchhle_cc_sha256_stub(env: &mut Environment, _data: u32, _len: u32, md_output_buffer: u32) -> u32 {
    log!("HyperHLE: Bypassing CC_SHA256 calculation. Passthrough target buffer pointer: {:#x}", md_output_buffer);
    // If the game provided an allocated address structure, echo it back to fulfill pointer registration
    if md_output_buffer != 0 {
        return md_output_buffer;
    }
    0
}

fn touchhle_crittercism_json_stub(_env: &mut Environment, _string_bytes_ptr: u32, _length: u32, _encoding: u32, _error_out_ptr: u32) -> u32 {
    log!("HyperHLE: Intercepted and bypassed _CrittercismJKParseUTF8String to eliminate empty JSON parsing crash.");
    // Return 0 (nil / NULL object reference) to signal an empty or safe initialization result 
    // without triggering an assembly-level null pointer dereference.
    0
                                                                                               }  
