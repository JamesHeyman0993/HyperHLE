/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CoreData framework stubs.

use crate::objc::{objc_classes, ClassExports, id};
use crate::frameworks::foundation::NSUInteger;
use crate::dyld::{ConstantExports, HostConstant}; 

// Satisfy the macro's need for a 'void' type
type void = ();

pub const CONSTANTS: ConstantExports = &[
    ("_NSSQLiteStoreType", HostConstant::NSString("NSSQLiteStoreType")),
    ("_NSInferMappingModelAutomaticallyOption", HostConstant::NSString("NSInferMappingModelAutomaticallyOption")),
    ("_NSMigratePersistentStoresAutomaticallyOption", HostConstant::NSString("NSMigratePersistentStoresAutomaticallyOption")),
];

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation NSManagedObjectContext : NSObject
    - (id)init { this }
    - (id)initWithConcurrencyType:(NSUInteger)_type { this }
    - (void)setPersistentStoreCoordinator:(id)_coordinator {}
    - (id)persistentStoreCoordinator { crate::objc::nil }
    - (id)persistentStore { crate::objc::nil }
    - (bool)save:(id)_error { true }
    // Unity often checks if the context is valid via this selector
    - (id)userInfo { crate::objc::nil }
    @end
    
    @implementation NSManagedObjectModel : NSObject
    - (id)init { this }
    - (id)initWithContentsOfURL:(id)_url { this }
    - (id)entities { crate::objc::nil }
    @end

    @implementation NSPersistentStoreCoordinator : NSObject
    - (id)initWithManagedObjectModel:(id)_model { this }
    - (id)addPersistentStoreWithType:(id)_type configuration:(id)_config URL:(id)_url options:(id)_options error:(id)_error {
        this 
    }
    @end

    @implementation NSManagedObject : NSObject
    - (id)initWithEntity:(id)_entity insertIntoManagedObjectContext:(id)_context { this }
    - (id)entity { crate::objc::nil }
    - (id)objectID { crate::objc::nil }
    @end

    @implementation NSEntityDescription : NSObject
    // FORCE FIX: Explicitly returning 'this'. 
    // If the log still says "-> nil", we must look for a duplicate stub in the foundation folder.
    + (id)entityForName:(id)_name inManagedObjectContext:(id)_context { 
        this 
    }
    + (id)insertNewObjectForEntityForName:(id)_name inManagedObjectContext:(id)_context {
        // Some Unity versions use this instead of entityForName
        this
    }
    - (id)init { this }
    - (id)name { crate::objc::nil }
    - (void)setProperties:(id)_properties {}
    - (id)properties { crate::objc::nil }
    @end

    @implementation NSAttributeDescription : NSObject
    - (id)init { this }
    - (void)setName:(id)_name {}
    - (void)setAttributeType:(NSUInteger)_type {}
    - (void)setOptional:(bool)_optional {}
    @end
};
