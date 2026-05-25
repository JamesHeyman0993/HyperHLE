/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `GLKit.framework` — Stubs for iOS 5.0+ graphics view controllers.

use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::objc::{
    id, msg, msg_super, nil, objc_classes, retain, release, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;

// --- GLKViewController ---
pub struct GLKViewControllerHostObject {
    preferred_frames_per_second: isize,
}
impl HostObject for GLKViewControllerHostObject {}

// --- GLKView ---
pub struct GLKViewHostObject {
    delegate: id,
    context: id,
}
impl HostObject for GLKViewHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);

@implementation GLKViewController : UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(GLKViewControllerHostObject {
        preferred_frames_per_second: 30,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    let this: id = msg_super![env; this init];
    log!("HyperHLE: GLKViewController initialized");
    this
}

- (isize)preferredFramesPerSecond {
    env.objc.borrow::<GLKViewControllerHostObject>(this).preferred_frames_per_second
}

- (())setPreferredFramesPerSecond:(isize)fps {
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).preferred_frames_per_second = fps;
}

- (())loadView {
    log!("HyperHLE: GLKViewController loadView generating implicit GLKView canvas.");
    
    let screen_bounds = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: 480.0, height: 320.0 },
    };

    let current_context: id = nil; 

    let glk_view_class = env.objc.lookup_class("GLKView").unwrap();
    let glk_view: id = msg![env; glk_view_class alloc];
    let glk_view: id = msg![env; glk_view initWithFrame:screen_bounds context:current_context];

    let _: () = msg![env; glk_view setDelegate:this];
    let _: () = msg![env; this setView:glk_view];
}

@end

@implementation GLKView : UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(GLKViewHostObject {
        delegate: nil,
        context: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame context:(id)context {
    let this: id = msg_super![env; this initWithFrame:frame];
    
    retain(env, context);
    env.objc.borrow_mut::<GLKViewHostObject>(this).context = context;
    
    log!("HyperHLE: GLKView initialized with context {:?}", context);
    this
}

- (id)delegate {
    env.objc.borrow::<GLKViewHostObject>(this).delegate
}

- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<GLKViewHostObject>(this).delegate = delegate;
}

- (id)context {
    env.objc.borrow::<GLKViewHostObject>(this).context
}

- (())setContext:(id)context {
    let old = env.objc.borrow::<GLKViewHostObject>(this).context;
    release(env, old); retain(env, context);
    env.objc.borrow_mut::<GLKViewHostObject>(this).context = context;
}

- (())display {
    log_dbg!("GLKView display requested");
    let (delegate, context) = {
        let h = env.objc.borrow::<GLKViewHostObject>(this);
        (h.delegate, h.context)
    };

    if delegate != nil {
        if let Some(sel) = env.objc.lookup_selector("glkView:drawInRect:") {
            let frame: CGRect = msg![env; this frame];
            let _: () = msg![env; delegate glkView:this drawInRect:frame];
        }
    }
}

@end
};
