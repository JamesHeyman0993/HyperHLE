/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 */
//! CoreData framework implementation.

use crate::dyld::{ConstantExports, FunctionExports, HostDylib};
use crate::objc::{ClassExports, ClassTemplate, TrivialHostObject};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreData.framework/CoreData",
    aliases: &[],
    class_exports: CLASSES,
    constant_exports: CONSTANTS,
    function_exports: FUNCTIONS,
};

const CLASSES: ClassExports = &[
    // CoreData relies heavily on these core classes
    ("NSManagedObject", ClassTemplate::Trivial),
    ("NSManagedObjectContext", ClassTemplate::Trivial),
    ("NSManagedObjectModel", ClassTemplate::Trivial),
    ("NSPersistentStoreCoordinator", ClassTemplate::Trivial),
    ("NSEntityDescription", ClassTemplate::Trivial),
    ("NSFetchRequest", ClassTemplate::Trivial),
];

const CONSTANTS: ConstantExports = &[
    // You'll often find error strings and notification names here
    ("NSManagedObjectContextObjectsDidChangeNotification", crate::dyld::HostConstant::NSString("NSManagedObjectContextObjectsDidChangeNotification")),
    ("NSSQLiteStoreType", crate::dyld::HostConstant::NSString("NSSQLite")),
];

const FUNCTIONS: FunctionExports = &[
    // Add C-style functions here if the app calls them directly
];
