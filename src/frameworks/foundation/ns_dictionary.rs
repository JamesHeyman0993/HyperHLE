/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSDictionary` class cluster, including `NSMutableDictionary`.

use super::ns_array::ArrayHostObject;
use super::ns_property_list_serialization::{
    deserialize_plist_from_file, NSPropertyListBinaryFormat_v1_0,
};
use super::ns_string::{from_rust_string, get_static_str, to_rust_string};
use super::{_nib_archive_decoder, ns_array, ns_keyed_unarchiver, ns_string, ns_url, NSUInteger};
use crate::abi::{CallFromHost, GuestFunction, VaList};
use crate::frameworks::core_foundation::{CFHashCode, CFIndex};
use crate::msg_super;
use crate::frameworks::foundation::ns_enumerator::{

    fast_enumeration_helper, NSFastEnumerationState,
};
use crate::frameworks::foundation::ns_file_manager::{
    NSFileModificationDate, NSFileSize, NSFileType,
};
use crate::fs::GuestPath;
use crate::mem::{ConstPtr, MutPtr, Ptr, SafeRead};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter, Class,
    ClassExports, HostObject, NSZonePtr,
};
use crate::{impl_HostObject_with_superclass, Environment};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// Alias for the return type of the `hash` method of the `NSObject` protocol.
type Hash = NSUInteger;

/// Belongs to _touchHLE_NSDictionary, also used by _touchHLE_NSSet
#[derive(Debug, Default)]
pub(super) struct DictionaryHostObject {
    /// Since we need custom hashing and custom equality, and these both need a
    /// `&mut Environment`, we can't just use a `HashMap<id, id>`.
    /// So here we are using a `HashMap` as a primitive for implementing a
    /// hash-map, which is not ideally efficient. :)
    /// The keys are the hash values, the values are a list of key-value pairs
    /// where the keys have the same hash value.
    pub(super) map: HashMap<Hash, Vec<(id, id)>>,
    pub(super) count: NSUInteger,
}
impl HostObject for DictionaryHostObject {}
impl DictionaryHostObject {
    pub(super) fn lookup(&self, env: &mut Environment, key: id) -> id {
        let hash: Hash = msg![env; key hash];
        let Some(collisions) = self.map.get(&hash) else {
            return nil;
        };
        for &(candidate_key, value) in collisions {
            if candidate_key == key || msg![env; candidate_key isEqual:key] {
                return value;
            }
        }
        nil
    }
    pub(super) fn insert(&mut self, env: &mut Environment, key: id, value: id, copy_key: bool) {
        let key: id = if copy_key {
            msg![env; key copy]
        } else {
            retain(env, key)
        };
        let hash: Hash = msg![env; key hash];

        let value = retain(env, value);
        let Some(collisions) = self.map.get_mut(&hash) else {
            self.map.insert(hash, vec![(key, value)]);
            self.count += 1;
            return;
        };
        for &mut (candidate_key, ref mut existing_value) in collisions.iter_mut() {
            if candidate_key == key || msg![env; candidate_key isEqual:key] {
                release(env, *existing_value);
                *existing_value = value;
                return;
            }
        }
        collisions.push((key, value));
        self.count += 1;
    }
    pub(super) fn remove(&mut self, env: &mut Environment, key: id) {
        let hash: Hash = msg![env; key hash];
        let Some(collisions) = self.map.get_mut(&hash) else {
            return;
        };
        let Some(idx) = collisions.iter().position(|&(candidate_key, _)| {
            candidate_key == key || msg![env; candidate_key isEqual:key]
        }) else {
            return;
        };
        let (existing_key, value) = collisions[idx];
        release(env, existing_key);
        release(env, value);
        collisions.remove(idx);
        self.count -= 1;
    }
    pub(super) fn release(&mut self, env: &mut Environment) {
        for collisions in self.map.values() {
            for &(key, value) in collisions {
                release(env, key);
                release(env, value);
            }
        }
    }
    pub(super) fn iter_keys(&self) -> impl Iterator<Item = id> + '_ {
        self.map.values().flatten().map(|&(key, _value)| key)
    }
}

// TODO: move those definitions to cf_dictionary.rs
#[repr(C, packed)]
pub struct CFDictionaryKeyCallBacks {
    pub version: CFIndex,         // version
    pub retain: GuestFunction,    // const void *(*retain)(CFAllocatorRef, const void *value)
    pub release: GuestFunction,   // void (*release)(CFAllocatorRef alloc, const void *val)
    pub copy_desc: GuestFunction, // CFStringRef (*copy_desc)(const void *val)
    pub equal: GuestFunction,     // Boolean (*equal)(const void *val1, const void *val2)
    pub hash: GuestFunction,      // CFHashCode (*hash)(const void *val)
}
unsafe impl SafeRead for CFDictionaryKeyCallBacks {}

#[repr(C, packed)]
pub struct CFDictionaryValueCallBacks {
    pub version: CFIndex,         // version
    pub retain: GuestFunction,    // const void *(*retain)(CFAllocatorRef, const void *value)
    pub release: GuestFunction,   // void (*release)(CFAllocatorRef alloc, const void *val)
    pub copy_desc: GuestFunction, // CFStringRef (*copy_desc)(const void *val)
    pub equal: GuestFunction,     // Boolean (*equal)(const void *val1, const void *val2)
}
unsafe impl SafeRead for CFDictionaryValueCallBacks {}

pub struct CFDictionaryHostObject {
    superclass: DictionaryHostObject,
    key_callbacks: CFDictionaryKeyCallBacks,
    value_callbacks: CFDictionaryValueCallBacks,
}
impl_HostObject_with_superclass!(CFDictionaryHostObject);
impl Default for CFDictionaryHostObject {
    fn default() -> Self {
        CFDictionaryHostObject {
            superclass: Default::default(),
            key_callbacks: CFDictionaryKeyCallBacks {
                version: 0,
                retain: GuestFunction::null_ptr(),
                release: GuestFunction::null_ptr(),
                copy_desc: GuestFunction::null_ptr(),
                equal: GuestFunction::null_ptr(),
                hash: GuestFunction::null_ptr(),
            },
            value_callbacks: CFDictionaryValueCallBacks {
                version: 0,
                retain: GuestFunction::null_ptr(),
                release: GuestFunction::null_ptr(),
                copy_desc: GuestFunction::null_ptr(),
                equal: GuestFunction::null_ptr(),
            },
        }
    }
}

impl CFDictionaryHostObject {
    fn lookup(&self, env: &mut Environment, key: id) -> id {
        let hash = self.hash(env, key);
        let Some(collisions) = self.superclass.map.get(&hash) else {
            return nil;
        };
        for &(candidate_key, value) in collisions {
            if self.equal_keys(env, candidate_key, key) {
                return value;
            }
        }
        nil
    }
    fn insert(&mut self, env: &mut Environment, key: id, value: id) {
        let hash = self.hash(env, key);
        let key = self.retain_key(env, key);
        let value = self.retain_value(env, value);
        self.superclass.count += 1;
        if let Entry::Vacant(e) = self.superclass.map.entry(hash) {
            e.insert(vec![(key, value)]);
            return;
        };
        self.remove(env, key);
        self.superclass
            .map
            .get_mut(&hash)
            .unwrap()
            .push((key, value));
    }
    fn remove(&mut self, env: &mut Environment, key: id) -> bool {
        let hash = self.hash(env, key);
        let Some(collisions) = self.superclass.map.get(&hash) else {
            return false;
        };
        let maybe_pos = collisions
            .iter()
            .position(|&(candidate_key, _)| self.equal_keys(env, candidate_key, key));
        if let Some(pos) = maybe_pos {
            let (existing_key, existing_value) =
                self.superclass.map.get_mut(&hash).unwrap().remove(pos);
            self.release_key(env, existing_key);
            self.release_value(env, existing_value);
            self.superclass.count -= 1;
            true
        } else {
            false
        }
    }
    fn hash(&self, env: &mut Environment, key: id) -> CFHashCode {
        let hash_func = self.key_callbacks.hash;
        if hash_func.to_ptr().is_null() {
            key.to_bits()
        } else {
            hash_func.call_from_host(env, (key,))
        }
    }
    fn equal_keys(&self, env: &mut Environment, key1: id, key2: id) -> bool {
        let equal_func = self.key_callbacks.equal;
        if equal_func.to_ptr().is_null() {
            key1 == key2
        } else {
            equal_func.call_from_host(env, (key1, key2))
        }
    }
    fn retain_key(&mut self, env: &mut Environment, key: id) -> id {
        let key_retain_func = self.key_callbacks.retain;
        if key_retain_func.to_ptr().is_null() {
            key
        } else {
            key_retain_func.call_from_host(env, (nil, key))
        }
    }
    fn release_key(&mut self, env: &mut Environment, key: id) {
        let key_release_func = self.key_callbacks.release;
        if !key_release_func.to_ptr().is_null() {
            key_release_func.call_from_host(env, (nil, key))
        }
    }
    fn retain_value(&mut self, env: &mut Environment, value: id) -> id {
        let value_retain_func = self.value_callbacks.retain;
        if value_retain_func.to_ptr().is_null() {
            value
        } else {
            value_retain_func.call_from_host(env, (nil, value))
        }
    }
    fn release_value(&mut self, env: &mut Environment, value: id) {
        let value_release_func = self.value_callbacks.release;
        if !value_release_func.to_ptr().is_null() {
            value_release_func.call_from_host(env, (nil, value))
        }
    }
}

pub fn init_with_objects_and_keys(
    env: &mut Environment,
    this: id,
    first_object: id,
    mut va_args: VaList,
) -> id {
    let first_key: id = va_args.next(env);
    
    let mut host_object = <DictionaryHostObject as Default>::default();

    // Safe handling: If the first key is nil, log a warning and return an empty dictionary shell
    if first_key == nil {
        log!("Warning: initWithObjectsAndKeys called with a nil key for object ({:?}). Bypassing initialization to prevent engine panic.", first_object);
        *env.objc.borrow_mut(this) = host_object;
        return this;
    }

    // Otherwise insert the valid first pair safely
    host_object.insert(env, first_key, first_object, /* copy_key: */ true);

    loop {
        let object: id = va_args.next(env);
        if object == nil {
            break;
        }
        let key: id = va_args.next(env);
        if key == nil {
            log!("Warning: Variadic initWithObjectsAndKeys sequence cut short due to a misaligned nil key reference.");
            break;
        }
        host_object.insert(env, key, object, /* copy_key: */ true);
    }

    *env.objc.borrow_mut(this) = host_object;
    this
}

fn init_with_dictionary_common(env: &mut Environment, this: id, other_dict: id) -> id {
    let mut host_object = <DictionaryHostObject as Default>::default();

    if other_dict != nil {
        let other_host_object: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(other_dict));
        for key in other_host_object.iter_keys() {
            let object = other_host_object.lookup(env, key);
            host_object.insert(env, key, object, /* copy_key: */ true);
        }
        *env.objc.borrow_mut(other_dict) = other_host_object;
    } else {
        log!("Warning: initWithDictionary: called with nil dictionary pointer — returning empty container safely");
    }

    *env.objc.borrow_mut(this) = host_object;
    this
}

fn init_with_objects_for_keys_common(env: &mut Environment, this: id, objects: id, keys: id) -> id {
    let keys_size: NSUInteger = msg![env; keys count];
    let objects_size: NSUInteger = msg![env; objects count];
    assert_eq!(keys_size, objects_size);

    let mut host_object = <DictionaryHostObject as Default>::default();

    let objects_enumerator: id = msg![env; objects objectEnumerator];
    let keys_enumerator: id = msg![env; keys objectEnumerator];

    loop {
        let next_key: id = msg![env; keys_enumerator nextObject];
        let next_object: id = msg![env; objects_enumerator nextObject];
        if next_key == nil {
            assert_eq!(next_object, nil);
            break;
        }
        host_object.insert(env, next_key, next_object, /* copy_key: */ true);
    }
    *env.objc.borrow_mut(this) = host_object;
    this
}

fn init_with_objects_for_keys_count_common(
    env: &mut Environment,
    this: id,
    objects: ConstPtr<id>,
    keys: ConstPtr<id>,
    count: NSUInteger,
) -> id {
    let mut host_object = <DictionaryHostObject as Default>::default();
    let keys_bits = keys.to_bits();
    let objects_bits = objects.to_bits();
    let elem_size = std::mem::size_of::<id>() as NSUInteger;
    for i in 0..count {
        let offset = i * elem_size;
        let key: id = env.mem.read(ConstPtr::from_bits(keys_bits + offset));
        let object: id = env.mem.read(ConstPtr::from_bits(objects_bits + offset));
        assert_ne!(key, nil);
        host_object.insert(env, key, object, /* copy_key: */ true);
    }

    *env.objc.borrow_mut(this) = host_object;
    this
}

fn all_keys_common(env: &mut Environment, this: id) -> id {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let keys: Vec<id> = host_obj
        .map
        .values()
        .flatten()
        .map(|&(key, _value)| key)
        .collect();
    *env.objc.borrow_mut(this) = host_obj;
    for &key in &keys {
        retain(env, key);
    }
    let res = ns_array::from_vec(env, keys);
    autorelease(env, res)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSDictionary: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    let base_class = env.objc.get_known_class("NSDictionary", &mut env.mem);
    
    if this == base_class {
        // If allocating the base class directly, route to our concrete implementation
        msg_class![env; _touchHLE_NSDictionary allocWithZone:zone]
    } else {
        // CRITICAL FIX: If a game SDK subclassed NSDictionary, forward the allocation 
        // up to NSObject so it actually allocates memory instead of looping infinitely!
        let superclass = env.objc.get_known_class("NSObject", &mut env.mem);
        msg_super![env; this allocWithZone:zone]
    
    }
}
    
+ (id)dictionary {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict init];
    autorelease(env, new_dict)
}

+ (id)dictionaryWithObject:(id)object forKey:(id)key {
    assert_ne!(key, nil);

    let new_dict = dict_from_keys_and_objects(env, &[(key, object)]);
    autorelease(env, new_dict)
}

+ (id)dictionaryWithObjectsAndKeys:(id)first_object, ...dots {
    let new_dict: id = msg![env; this alloc];
    let new_dict = init_with_objects_and_keys(env, new_dict, first_object, dots.start());
    autorelease(env, new_dict)
}

+ (id)dictionaryWithContentsOfFile:(id)path {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict initWithContentsOfFile:path];
    autorelease(env, new_dict)
}
+ (id)dictionaryWithContentsOfURL:(id)url {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict initWithContentsOfURL:url];
    autorelease(env, new_dict)
}

+ (id)dictionaryWithObjects:(id)objects forKeys:(id)keys {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict initWithObjects:objects forKeys:keys];
    autorelease(env, new_dict)
}

+ (id)dictionaryWithObjects:(ConstPtr<id>)objects
                    forKeys:(ConstPtr<id>)keys
                      count:(NSUInteger)count {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict initWithObjects:objects forKeys:keys count:count];
    autorelease(env, new_dict)
}

+ (id)dictionaryWithDictionary:(id)dict {
    let new_dict: id = msg![env; this alloc];
    let new_dict: id = msg![env; new_dict initWithDictionary:dict];
    autorelease(env, new_dict)
}

- (id)init {
    todo!("TODO: Implement [dictionary init] for custom subclasses")
}

- (id)keyEnumerator {
    let keys: id = msg![env; this allKeys];
    msg![env; keys objectEnumerator]
}

- (id)objectEnumerator {
    let values: id = msg![env; this allValues];
    msg![env; values objectEnumerator]
}

- (id)initWithContentsOfFile:(id)path {
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ false,
    )
}
- (id)initWithContentsOfURL:(id)url {
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    deserialize_plist_from_file(env, &path, /* array_expected: */ false)
}

- (bool)writeToFile:(id)path atomically:(bool)atomically {
    let error_desc: MutPtr<id> = Ptr::null();
    let data: id = msg_class![env; NSPropertyListSerialization
            dataFromPropertyList:this
                          format:NSPropertyListBinaryFormat_v1_0
                errorDescription:error_desc];
    let res = msg![env; data writeToFile:path atomically:atomically];
    log_dbg!(
        "[(NSDictionary *){:?} writeToFile:{:?} atomically:{}] -> {}",
        this,
        to_rust_string(env, path),
        atomically,
        res
    );
    res
}

- (id)valueForKey:(id)key {
    let key_str = to_rust_string(env, key);
    assert!(!key_str.starts_with('@'));
    msg![env; this objectForKey:key]
}

- (id)fileModificationDate {
    let modif_date_key = get_static_str(env, NSFileModificationDate);
    msg![env; this objectForKey:modif_date_key]
}
- (u64)fileSize {
    let size_key = get_static_str(env, NSFileSize);
    let num = msg![env; this objectForKey:size_key];
    if num != nil {
        msg![env; num unsignedLongLongValue]
    } else {
        0
    }
}
- (id)fileType {
    let file_type_key = get_static_str(env, NSFileType);
    msg![env; this objectForKey:file_type_key]
}

- (())enumerateKeysAndObjectsUsingBlock:(id)block {
    if block.is_null() {
        return;
    }
    let block_bits = block.to_bits();
    let block_impl: GuestFunction = env.mem.read(ConstPtr::from_bits(block_bits + 12));
    if block_impl.to_ptr().is_null() {
        return;
    }

    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let pairs: Vec<(id, id)> = host_obj.map.values().flatten().copied().collect();
    *env.objc.borrow_mut(this) = host_obj;

    let mut stop: bool = false;
    let stop_ptr: MutPtr<bool> = env.mem.alloc(1).cast();
    
    for (k, v) in pairs {
        env.mem.write(stop_ptr, false);
        let _: () = block_impl.call_from_host(env, (block, k, v, stop_ptr));
        stop = env.mem.read(stop_ptr);
        if stop {
            break;
        }
    }
}

@end

@implementation NSMutableDictionary: NSDictionary

+ (id)allocWithZone:(NSZonePtr)zone {
    let base_mutable_class = env.objc.get_known_class("NSMutableDictionary", &mut env.mem);
    
    if this == base_mutable_class {
        msg_class![env; _touchHLE_NSMutableDictionary allocWithZone:zone]
    } else {
        // Forward up to the parent class structure safely
        let superclass = env.objc.get_known_class("NSDictionary", &mut env.mem);
        msg_super![env; this allocWithZone:zone]
        
    }
}
    
+ (id)dictionaryWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

- (id)initWithContentsOfFile:(id)path {
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    let tmp = deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ false
    );
    if tmp == nil {
        return nil;
    }
    let res = msg_class![env; NSMutableDictionary alloc];
    let res = msg![env; res initWithDictionary:tmp];
    release(env, tmp);
    res
}
- (id)initWithContentsOfURL:(id)url {
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    let tmp = deserialize_plist_from_file(env, &path, /* array_expected: */ false);
    if tmp == nil {
        return nil;
    }
    let res = msg_class![env; NSMutableDictionary alloc];
    let res = msg![env; res initWithDictionary:tmp];
    release(env, tmp);
    res
}

- (())removeObjectsForKeys:(id)key_array {
    if key_array == nil {
        return;
    }
    let count: NSUInteger = msg![env; key_array count];
    for i in 0..count {
        let key: id = msg![env; key_array objectAtIndex:i];
        () = msg![env; this removeObjectForKey:key];
    }
}

@end

@implementation _touchHLE_NSDictionary: NSDictionary

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<DictionaryHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    std::mem::take(env.objc.borrow_mut::<DictionaryHostObject>(this)).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
    }

- (id)init {
    *env.objc.borrow_mut(this) = <DictionaryHostObject as Default>::default();
    this
}

 - (id)initWithObjectsAndKeys:(id)first_object, ...dots {
    init_with_objects_and_keys(env, this, first_object, dots.start())
 }
    
- (id)initWithDictionary:(id)dictionary {
    init_with_dictionary_common(env, this, dictionary)
}

- (id)initWithObjects:(id)objects forKeys:(id)keys {
    init_with_objects_for_keys_common(env, this, objects, keys)
}

- (id)initWithObjects:(ConstPtr<id>)objects
              forKeys:(ConstPtr<id>)keys
                count:(NSUInteger)count {
    init_with_objects_for_keys_count_common(env, this, objects, keys, count)
}

- (NSUInteger)count {
    env.objc.borrow::<DictionaryHostObject>(this).count
}
- (id)objectForKey:(id)key {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let res = host_obj.lookup(env, key);
    *env.objc.borrow_mut(this) = host_obj;
    res
}

- (id)allKeys {
    all_keys_common(env, this)
}

- (id)allValues {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let values: Vec<id> = host_obj.map.values().flatten().map(|&(_key, value)| value).collect();
    *env.objc.borrow_mut(this) = host_obj;
    for &val in &values {
        retain(env, val);
    }
    let res = crate::frameworks::foundation::ns_array::from_vec(env, values);
    autorelease(env, res)
}

- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    let objects: id = msg![env; this allKeys];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let mut_dict: id = msg_class![env; NSMutableDictionary alloc];
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    for (k, v) in host_obj.map.values().flatten() {
        () = msg![env; mut_dict setObject:(*v) forKey:(*k)];
    }
    *env.objc.borrow_mut(this) = host_obj;
    mut_dict
}

- (id)description {
    build_description(env, this)
}

- ((()))encodeWithCoder:(id)coder {
    let class: Class = msg![env; coder class];
    let keyed_arch_class: Class = msg_class![env; NSKeyedArchiver class];

    if env.objc.class_is_subclass_of(class, keyed_arch_class) {
        let host = env.objc.borrow::<DictionaryHostObject>(this);
        let pairs: Vec<(id, id)> = host.map.values()
           .flat_map(|v| v.iter().copied())
           .collect();
        drop(host);

        let keys_array: id = msg_class![env; NSMutableArray new];
        let objects_array: id = msg_class![env; NSMutableArray new];
        for (k, v) in &pairs {
            let key = *k;
            let val = *v;
            () = msg![env; keys_array addObject:key];
            () = msg![env; objects_array addObject:val];
        }

        let keys_str = from_rust_string(env, "NS.keys".to_string());
        let objects_str = from_rust_string(env, "NS.objects".to_string());
        () = msg![env; coder encodeObject:keys_array forKey:keys_str];
        () = msg![env; coder encodeObject:objects_array forKey:objects_str];

        release(env, keys_str);
        release(env, objects_str);
        release(env, keys_array);
        release(env, objects_array);
    } else {
        log!("Intercepted immutable encodeWithCoder: stub processing for class {:?}", class);
    }
}

@end

@implementation _touchHLE_NSMutableDictionary: NSMutableDictionary

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<DictionaryHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    std::mem::take(env.objc.borrow_mut::<DictionaryHostObject>(this)).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (())setDictionary:(id)dict {
    todo_objc_setter!(this, dict);
}

- (id)initWithObjectsAndKeys:(id)first_object, ...dots {
    init_with_objects_and_keys(env, this, first_object, dots.start())
}

- (id)initWithDictionary:(id)dictionary {
    init_with_dictionary_common(env, this, dictionary)
}

- (id)init {
    *env.objc.borrow_mut(this) = <DictionaryHostObject as Default>::default();
    this
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    msg![env; this init]
}

- (id)initWithCoder:(id)coder {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _touchHLE_NIBArchiveDecoder class];
    let tuples = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_dict(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_dict(env, coder)
    } else {
        unimplemented!()
    };
    release(env, this);
    let dict = dict_from_keys_and_objects(env, &tuples);

    let mut_dict = msg![env; dict mutableCopy];
    release(env, dict);
    mut_dict
}

- (id)initWithObjects:(id)objects forKeys:(id)keys {
    init_with_objects_for_keys_common(env, this, objects, keys)
}

- (id)initWithObjects:(ConstPtr<id>)objects
              forKeys:(ConstPtr<id>)keys
                count:(NSUInteger)count {
    init_with_objects_for_keys_count_common(env, this, objects, keys, count)
}

- (NSUInteger)count {
    env.objc.borrow::<DictionaryHostObject>(this).count
}
- (id)objectForKey:(id)key {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let res = host_obj.lookup(env, key);
    *env.objc.borrow_mut(this) = host_obj;
    res
}

- ((()))encodeWithCoder:(id)coder {
    let class: Class = msg![env; coder class];
    let keyed_arch_class: Class = msg_class![env; NSKeyedArchiver class];

    if env.objc.class_is_subclass_of(class, keyed_arch_class) {
        let host = env.objc.borrow::<DictionaryHostObject>(this);
        let pairs: Vec<(id, id)> = host.map.values()
           .flat_map(|v| v.iter().copied())
           .collect();
        drop(host);

        let keys_array: id = msg_class![env; NSMutableArray new];
        let objects_array: id = msg_class![env; NSMutableArray new];
        for (k, v) in &pairs {
            let key = *k;
            let val = *v;
            () = msg![env; keys_array addObject:key];
            () = msg![env; objects_array addObject:val];
        }

        let keys_str = from_rust_string(env, "NS.keys".to_string());
        let objects_str = from_rust_string(env, "NS.objects".to_string());
        () = msg![env; coder encodeObject:keys_array forKey:keys_str];
        () = msg![env; coder encodeObject:objects_array forKey:objects_str];

        release(env, keys_str);
        release(env, objects_str);
        release(env, keys_array);
        release(env, objects_array);
    } else {
        log!("Intercepted mutable encodeWithCoder: stub processing for class {:?}", class);
    }
}

- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    let objects: id = msg![env; this allKeys];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

- (id)copyWithZone:(NSZonePtr)_zone {
    let entries: Vec<_> =
        env.objc.borrow_mut::<DictionaryHostObject>(this).map.values().flatten().copied().collect();
    dict_from_keys_and_objects(env, &entries)
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let mut_dict: id = msg_class![env; NSMutableDictionary alloc];
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    for (k, v) in host_obj.map.values().flatten() {
        () = msg![env; mut_dict setObject:(*v) forKey:(*k)];
    }
    *env.objc.borrow_mut(this) = host_obj;
    mut_dict
}

- (())setValue:(id)value forKey:(id)key {
    if value == nil {
        msg![env; this removeObjectForKey:key]
    } else {
        msg![env; this setObject:value forKey:key]
    }
}

- (())setObject:(id)object forKey:(id)key {
        let mut object_to_insert = object;

        if object == nil {
            let key_str = if key != nil {
                crate::frameworks::foundation::ns_string::to_rust_string(env, key).to_string()
            } else {
                "nil".to_string()
            };
            
            if key_str.contains("Id") || key_str.contains("ID") || key_str.contains("crossPublisher") {
                log!("HACK: Intercepted and substituted nil object for tracking payload identity key: '{}'", key_str);
                let dummy = crate::frameworks::foundation::ns_string::from_rust_string(
                    env,
                    "touchHLE-safe-dummy-id-123456789".to_string(),
                );
                object_to_insert = autorelease(env, dummy);
            } else {
                log!("Warning: [NSMutableDictionary setObject:forKey:] attempt to insert nil object for key {} — ignoring", key_str);
                return;
            }
        }

        if key == nil {
            log!("Warning: [NSMutableDictionary setObject:forKey:] attempt to use nil key — ignoring");
            return;
        }

        let mut host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
        host_obj.insert(env, key, object_to_insert, /* copy_key: */ true);
        *env.objc.borrow_mut(this) = host_obj;
    }

- (())removeObjectForKey:(id)key {
    if key.is_null() {
        log!("Warning: [NSMutableDictionary removeObjectForKey:] key is nil — ignored");
        return;
    }
    let mut host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.remove(env, key);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeAllObjects {
    let mut old_host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    old_host_obj.release(env);
}

- (())addEntriesFromDictionary:(id)other {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(other));
    for (k, v) in host_obj.map.values().flatten() {
        () = msg![env; this setObject:(*v) forKey:(*k)];
    }
    *env.objc.borrow_mut(other) = host_obj;
}

- (id)description {
    build_description(env, this)
}

- (id)allKeys {
    all_keys_common(env, this)
}

- (id)allValues {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let values: Vec<id> = host_obj.map.values().flatten().map(|&(_key, value)| value).collect();
    *env.objc.borrow_mut(this) = host_obj;
    for &val in &values {
        retain(env, val);
    }
    let res = ns_array::from_vec(env, values);
    autorelease(env, res)
}

- (id)allKeysForObject:(id)obj {
    let res: id = msg_class![env; NSMutableArray new];

    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.map.values().flatten().for_each(|&(key, value)| {
        let equal = msg![env; obj isEqual:value];
        if equal {
            () = msg![env; res addObject:key];
        }
    });
    *env.objc.borrow_mut(this) = host_obj;

    let res_imm = msg![env; res copy];
    release(env, res);
    autorelease(env, res_imm)
}

- (id)objectEnumerator {
    let values: id = msg![env; this allValues];
    msg![env; values objectEnumerator]
}

- (())enumerateKeysAndObjectsUsingBlock:(id)block {
    if block.is_null() {
        return;
    }
    let block_bits = block.to_bits();
    let block_impl: GuestFunction = env.mem.read(ConstPtr::from_bits(block_bits + 12));
    if block_impl.to_ptr().is_null() {
        return;
    }

    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let pairs: Vec<(id, id)> = host_obj.map.values().flatten().copied().collect();
    *env.objc.borrow_mut(this) = host_obj;

    let mut stop: bool = false;
    let stop_ptr: MutPtr<bool> = env.mem.alloc(1).cast();
    
    for (k, v) in pairs {
        env.mem.write(stop_ptr, false);
        let _: () = block_impl.call_from_host(env, (block, k, v, stop_ptr));
        stop = env.mem.read(stop_ptr);
        if stop {
            break;
        }
    }
}

@end
    @implementation _touchHLE_NSMutableDictionary_non_retaining: _touchHLE_NSMutableDictionary

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<CFDictionaryHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithKeyCallbacks:(ConstPtr<CFDictionaryKeyCallBacks>)key_callbacks
         andValueCallbacks:(ConstPtr<CFDictionaryValueCallBacks>)value_callbacks {
    if !key_callbacks.is_null() {
        assert!(!value_callbacks.is_null());
        let host_object = env.objc.borrow_mut::<CFDictionaryHostObject>(this);
        host_object.key_callbacks = env.mem.read(key_callbacks);
        host_object.value_callbacks = env.mem.read(value_callbacks);
    };
    this
}

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithObjectsAndKeys:(id)_first_object, ..._dots {
    todo!();
}
- (id)description {
    todo!();
}
- (id)copyWithZone:(NSZonePtr)_zone {
    todo!();
}
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    todo!();
}

- (id)objectForKey:(id)key {
    let host_obj: CFDictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let res = host_obj.lookup(env, key);
    *env.objc.borrow_mut(this) = host_obj;
    res
}

- (id)valueForKey:(id)_key {
    panic!("Unexpected call to valueForKey: for _touchHLE_NSMutableDictionary_non_retaining object {this:?}");
}

- (())setObject:(id)object extern_key:(id)key {
    if object == nil {
        log!("Warning: [_touchHLE_NSMutableDictionary_non_retaining setObject:forKey:] attempt to insert nil object — ignoring");
        return;
    }
    if key == nil {
        log!("Warning: [_touchHLE_NSMutableDictionary_non_retaining setObject:forKey:] attempt to use nil key — ignoring");
        return;
    }

    let mut host_obj: CFDictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.insert(env, key, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeObjectForKey:(id)key {
    if key == nil {
        log!("Warning: [_touchHLE_NSMutableDictionary_non_retaining removeObjectForKey:] key is nil — ignored");
        return;
    }

    let mut host_obj: CFDictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.remove(env, key);
    *env.objc.borrow_mut(this) = host_obj;
}

- (id)allKeys {
    let host_obj: DictionaryHostObject = std::mem::take(env.objc.borrow_mut(this));
    let keys: Vec<id> = host_obj.map.values().flatten().map(|&(key, _value)| key).collect();
    *env.objc.borrow_mut(this) = host_obj;

    let array: id = msg_class![env; _touchHLE_NSArray_non_retaining alloc];
    env.objc.borrow_mut::<ArrayHostObject>(array).array = keys;
    array
}

@end

};

pub fn dict_from_keys_and_objects(env: &mut Environment, keys_and_objects: &[(id, id)]) -> id {
    let dict: id = msg_class![env; NSDictionary alloc];

    let mut host_object = <DictionaryHostObject as Default>::default();
    for &(key, object) in keys_and_objects {
        host_object.insert(env, key, object, /* copy_key: */ true);
    }
    *env.objc.borrow_mut(dict) = host_object;

    dict
}

pub fn mutable_dict_from_keys_and_objects(
    env: &mut Environment,
    keys_and_objects: &[(id, id)],
) -> id {
    let dict: id = msg_class![env; NSMutableDictionary alloc];

    let mut host_object = <DictionaryHostObject as Default>::default();
    for &(key, object) in keys_and_objects {
        host_object.insert(env, key, object, /* copy_key: */ true);
    }
    *env.objc.borrow_mut(dict) = host_object;

    dict
}

fn build_description(env: &mut Environment, dict: id) -> id {
    let desc: id = msg_class![env; NSMutableString new];
    let prefix: id = from_rust_string(env, "{\n".to_string());
    () = msg![env; desc appendString:prefix];
    release(env, prefix);
    let keys: Vec<id> = env
        .objc
        .borrow_mut::<DictionaryHostObject>(dict)
        .iter_keys()
        .collect();
    for key in keys {
        let key_desc: id = msg![env; key description];
        let value: id = msg![env; dict objectForKey:key];
        let val_desc: id = msg![env; value description];
        let format = format!(
            "\t{} = {};\n",
            to_rust_string(env, key_desc),
            to_rust_string(env, val_desc)
        );
        let format = from_rust_string(env, format);
        () = msg![env; desc appendString:format];
        release(env, format);
    }
    let suffix: id = from_rust_string(env, "}".to_string());
    () = msg![env; desc appendString:suffix];
    release(env, suffix);
    let desc_imm = msg![env; desc copy];
    release(env, desc);
    autorelease(env, desc_imm)
}
