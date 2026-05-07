/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 */
//! CoreData framework implementation.

use crate::dyld::{ConstantExports, FunctionExports, HostConstant, HostDylib};
use crate::objc::{ClassExports, ClassTemplate};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreData.framework/CoreData",
    aliases: &[],
    class_exports: &[CLASSES],       // Added &[ ] to make it a slice of slices
    constant_exports: &[CONSTANTS], // Added &[ ] to make it a slice of slices
    function_exports: &[FUNCTIONS], // Added &[ ] to make it a slice of slices
};

// Note: ClassTemplate::Trivial likely doesn't exist as an enum variant.
// In touchHLE, a "trivial" class is usually an empty template.
const CLASSES: ClassExports = &[
    ("NSManagedObject", ClassTemplate::EMPTY),
    ("NSManagedObjectContext", ClassTemplate::EMPTY),
    ("NSManagedObjectModel", ClassTemplate::EMPTY),
    ("NSPersistentStoreCoordinator", ClassTemplate::EMPTY),
    ("NSEntityDescription", ClassTemplate::EMPTY),
    ("NSFetchRequest", ClassTemplate::EMPTY),
];

const CONSTANTS: ConstantExports = &[
    ("NSSQLiteStoreType", HostConstant::NSString("NSSQLite")),
];

const FUNCTIONS: FunctionExports = &[];
