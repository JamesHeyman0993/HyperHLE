/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CoreData framework stubs.

use crate::objc::{objc_classes, ClassExports, id, void, bool};
use crate::frameworks::foundation::NSUInteger;

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation NSManagedObjectContext : NSObject
    - (id)init { this }
    - (id)initWithConcurrencyType:(NSUInteger)_type { this }
    - (void)setPersistentStoreCoordinator:(id)_coordinator {}
    - (id)persistentStoreCoordinator { crate::objc::nil }
    - (id)persistentStore { crate::objc::nil }
    - (bool)save:(id)_error { true }
    @end
    
    @implementation NSManagedObjectModel : NSObject
    - (id)init { this }
    - (id)initWithContentsOfURL:(id)_url { this }
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
    + (id)entityForName:(id)_name inManagedObjectContext:(id)_context { crate::objc::nil }
    - (id)name { crate::objc::nil }
    @end
};
