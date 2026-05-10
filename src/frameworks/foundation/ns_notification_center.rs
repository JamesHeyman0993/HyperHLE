/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSNotificationCenter`.

use super::ns_notification::NSNotificationName;
use super::ns_string;

use crate::objc::{
    id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub default_center: Option<id>,
}

#[derive(Clone)]
struct Observer {
    observer: id,
    selector: SEL,
    object: id,
}

struct NSNotificationCenterHostObject {
    observers: HashMap<Cow<'static, str>, Vec<Observer>>,
}
impl HostObject for NSNotificationCenterHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSNotificationCenter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSNotificationCenterHostObject {
        observers: HashMap::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)defaultCenter {
    if let Some(c) = env.framework_state.foundation.ns_notification_center.default_center {
        c
    } else {
        // Fix: Use alloc/init instead of new to ensure metaclass compatibility
        let cls = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
        let instance: id = msg![env; cls alloc];
        let new: id = msg![env; instance init];
        
        env.framework_state.foundation.ns_notification_center.default_center = Some(new);
        new
    }
}

- (())dealloc {
    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    let observers = std::mem::take(&mut host_obj.observers);
    for observer in observers.values().flatten() {
        release(env, observer.object);
    }
    env.objc.dealloc_object(this, &mut env.mem);
}

- (())addObserver:(id)observer
         selector:(SEL)selector
             name:(NSNotificationName)name
           object:(id)object {
    
    // Game-specific hack for Cut the Rope
    if name != nil &&
        env.bundle.bundle_identifier().starts_with("com.chillingo.cuttherope") &&
        selector == env.objc.lookup_selector("fetchUpdateNotification:").unwrap() {
        log!("Applying game-specific hack for Cut the Rope: ignoring addObserver");
        return;
    }

    // Fix: Handle case where name is nil (Observer wants ALL notifications)
    let name_key = if name == nil {
        Cow::Borrowed("__TOUCHHLE_ALL_NOTIFICATIONS__")
    } else {
        ns_string::to_rust_string(env, name)
    };

    log_dbg!(
        "[(NSNotificationCenter*){:?} addObserver:{:?} selector:{:?} name:{:?} object:{:?}",
        this, observer, selector, name_key, object,
    );

    retain(env, object);

    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    host_obj.observers.entry(name_key).or_default().push(Observer {
        observer,
        selector,
        object,
    });
}

- (())removeObserver:(id)observer {
    msg![env; this removeObserver:observer name:nil object:nil]
}

- (())removeObserver:(id)observer
                name:(NSNotificationName)name
              object:(id)object {
    assert!(observer != nil);

    let name_opt = if name == nil {
        None
    } else {
        Some(ns_string::to_rust_string(env, name))
    };

    log_dbg!(
        "[(NSNotificationCenter*){:?} removeObserver:{:?} name:{:?} object:{:?}",
        this, observer, name_opt, object,
    );

    let mut removed_observers = Vec::new();
    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    
    if let Some(ref name_str) = name_opt {
        if let Some(observers) = host_obj.observers.get_mut(name_str) {
            remove_observers_internal(observers, &mut removed_observers, observer, object);
        }
    } else {
        // If name is nil, remove this observer from ALL notification buckets
        for observers in host_obj.observers.values_mut() {
            remove_observers_internal(observers, &mut removed_observers, observer, object);
        }
    }

    for removed_observer in removed_observers {
        release(env, removed_observer.object);
    }
}

- (())postNotification:(id)notification {
    let name_id: id = msg![env; notification name];
    let name_str = ns_string::to_rust_string(env, name_id);
    let notification_poster: id = msg![env; notification object];

    log_dbg!("Posting notification: {:?} from {:?}", name_str, notification_poster);

    // We need to collect matching observers to avoid borrow checker issues 
    // while iterating and potentially modifying the map.
    let mut targets = Vec::new();
    {
        let host_obj = env.objc.borrow::<NSNotificationCenterHostObject>(this);
        
        // 1. Get observers specifically for this name
        if let Some(observers) = host_obj.observers.get(&name_str) {
            targets.extend(observers.clone());
        }
        
        // 2. Get observers listening to "ALL" notifications
        if let Some(all_observers) = host_obj.observers.get("__TOUCHHLE_ALL_NOTIFICATIONS__") {
            targets.extend(all_observers.clone());
        }
    }

    for Observer { observer, selector, object } in targets {
        // Filter by poster object if specified
        if object != nil && notification_poster != object {
            continue;
        }

        retain(env, observer);
        let _: () = msg_send(env, (observer, selector, notification));
        release(env, observer);
    }
}

- (())postNotificationName:(NSNotificationName)name
                    object:(id)object {
    msg![env; this postNotificationName:name object:object userInfo:nil]
}

- (())postNotificationName:(NSNotificationName)name
                    object:(id)object
                  userInfo:(id)user_info {
    let notification_cls = env.objc.get_known_class("NSNotification", &mut env.mem);
    let instance: id = msg![env; notification_cls alloc];
    let notification: id = msg![env; instance initWithName:name object:object userInfo:user_info];
    
    let _: () = msg![env; this postNotification:notification];
    release(env, notification);
}

// Universal Fix: Helper for the emulator to fake system events
- (())_touchHLE_postSystemNotification:(id)name_rust_str {
    let name_nss = ns_string::from_rust_string(env, name_rust_str);
    let _: () = msg![env; this postNotificationName:name_nss object:nil];
}

@end

};

fn remove_observers_internal(
    observers: &mut Vec<Observer>,
    removed_observers: &mut Vec<Observer>,
    observer: id,
    object: id,
) {
    let mut i = 0;
    while i < observers.len() {
        if observers[i].observer == observer && (object == nil || object == observers[i].object) {
            removed_observers.push(observers.swap_remove(i));
        } else {
            i += 1;
        }
    }
}
