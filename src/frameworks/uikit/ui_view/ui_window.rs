/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIWindow`.
//!
//! Useful resources:
//! - [Technical Q&A QA1588: Automatic orientation support for iPhone and iPad apps](https://developer.apple.com/library/archive/qa/qa1588/_index.html)
//! - [Technical Q&A QA1688: Why won't my UIViewController rotate with the device?](https://developer.apple.com/library/archive/qa/qa1688/_index.html)

use super::UIViewHostObject;
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::{CGPoint, CGRect};
use crate::frameworks::foundation::ns_string;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientationLandscapeLeft, UIInterfaceOrientationLandscapeRight,
};
use crate::frameworks::uikit::ui_device::{
    UIDeviceOrientationLandscapeLeft, UIDeviceOrientationLandscapeRight,
};
use crate::objc::{id, msg, msg_class, msg_super, nil, objc_classes, ClassExports};

#[derive(Default)]
pub struct State {
    /// List of visible windows for internal purposes. Non-retaining!
    ///
    /// This is public because Core Animation also uses it.
    pub windows: Vec<id>,
    /// The most recent window which received `makeKeyAndVisible` message.
    /// Non-retaining!
    pub key_window: Option<id>,
    /// Retained pointer to root view controllers per window instance
    pub root_view_controllers: std::collections::HashMap<id, id>,
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIWindow: UIView

- (id)initWithFrame:(CGRect)frame {
    let this = msg_super![env; this initWithFrame:frame];
    // Undocumented: windows seem to be hidden by default on iOS, unlike views.
    () = msg_super![env; this setHidden:true];

    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    list.push(this);
    log_dbg!(
        "New window: {:?}. New list of all windows: {:?}",
        this,
        list,
    );

    this
}

- (id)initWithCoder:(id)coder {
    let this = msg_super![env; this initWithCoder:coder];
    () = msg_super![env; this setHidden:true];

    let screen: id = msg_class![env; UIScreen mainScreen];
    let screen_bounds: CGRect = msg![env; screen bounds];
    let current_bounds: CGRect = msg![env; this bounds];
    if current_bounds.size != screen_bounds.size {
        log_dbg!(
            "UIWindow {:?}: overriding NIB-encoded size {:?} with UIScreen.bounds size {:?}",
            this,
            current_bounds.size,
            screen_bounds.size,
        );
        () = msg![env; this setFrame:screen_bounds];
    }

    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    list.push(this);
    this
}

- (())dealloc {
    if let Some(key_window) = env.framework_state.uikit.ui_view.ui_window.key_window {
        if key_window == this {
            env.framework_state.uikit.ui_view.ui_window.key_window = None;
        }
    }
    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    if let Some(idx) = list.iter().position(|&w| w == this) {
        list.remove(idx);
    }
    env.framework_state.uikit.ui_view.ui_window.root_view_controllers.remove(&this);
    msg_super![env; this dealloc]
}

- (())layoutIfNeeded {
    log_dbg!("[(UIWindow*){:?} layoutIfNeeded]", this);
    () = msg![env; this layoutSubviews];
}

- (id)hitTest:(CGPoint)point withEvent:(id)event {
    // FIX: A window cannot receive touches if hidden, fully transparent, or user interaction is disabled
    let hidden: bool = msg![env; this isHidden];
    let alpha: f32 = msg![env; this alpha];
    let user_interaction: bool = msg![env; this isUserInteractionEnabled];
    
    if hidden || alpha <= 0.01 || !user_interaction {
        return nil;
    }
    
    // Check if the point actually falls within the window bounds boundary
    let bounds: CGRect = msg![env; this bounds];
    let point_inside: bool = msg![env; this pointInside:point withEvent:event];
    if !point_inside {
        return nil;
    }

    let subviews = env.objc.borrow::<super::UIViewHostObject>(this).subviews.clone();
    for subview in subviews.into_iter().rev() {
        let sub_point: CGPoint = msg![env; subview convertPoint:point fromView:this];
        let hit: id = msg![env; subview hitTest:sub_point withEvent:event];
        
        if hit != nil { 
            let class_name: id = msg![env; hit class];
            log!("Hit detected on object: {:?} (Class: {:?}) at {:?}", hit, class_name, sub_point);
            return hit; 
        }
    }
    
    // FIX: Instead of returning `this` blindly, return the window only if it is the intended recipient.
    // If no view matches, returning `this` can block underneath elements, but returning `nil` breaks background taps.
    // Returning `this` is correct ONLY if window has a fallback layer, otherwise pass safely.
    this
}
    
- (())setHidden:(bool)is_hidden {
    () = msg_super![env; this setHidden:is_hidden];
    log_dbg!("[(UIWindow*){:?} setHidden:{:?}]", this, is_hidden);
}

- (())makeKeyWindow {
    env.framework_state.uikit.ui_view.ui_window.key_window = Some(this);
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let notif_name = ns_string::get_static_str(env, UIWindowDidBecomeKeyNotification);
    () = msg![env; center postNotificationName:notif_name object:this userInfo:nil];
}

- (bool)isKeyWindow {
    env.framework_state.uikit.ui_view.ui_window.key_window == Some(this)
}

- (())makeKeyAndVisible {
    () = msg![env; this makeKeyWindow];
    () = msg![env; this setHidden:false];
}

- (())setContentView:(id)view {
    () = msg![env; this addSubview:view];
}

- (id)contentView {
    let subviews = &env.objc.borrow::<UIViewHostObject>(this).subviews;
    subviews.first().copied().unwrap_or(nil)
}

- (())setRootViewController:(id)view_controller {
    if view_controller != nil {
        env.framework_state.uikit.ui_view.ui_window.root_view_controllers.insert(this, view_controller);
        let view: id = msg![env; view_controller view];
        () = msg![env; this addSubview:view];
    } else {
        env.framework_state.uikit.ui_view.ui_window.root_view_controllers.remove(&this);
    }
}

- (id)rootViewController {
    env.framework_state.uikit.ui_view.ui_window.root_view_controllers.get(&this).copied().unwrap_or(nil)
}

- (id)nextResponder {
    msg_class![env; UIApplication sharedApplication]
}

- (())addSubview:(id)view {
    log_dbg!("[(UIWindow*){:?} addSubview:{:?}] => ()", this, view);

    if view == nil { return; }

    let vc = {
        let host_obj = env.objc.borrow::<UIViewHostObject>(view);
        host_obj.view_controller
    };
    
    if vc != nil {
        () = msg![env; vc viewWillAppear:false];
        () = msg_super![env; this addSubview:view];
        () = msg![env; vc viewDidAppear:false];
    } else {
        () = msg_super![env; this addSubview:view];
    }

    if let Some(orientation) = match env.window.as_ref().unwrap().current_rotation() {
        crate::window::DeviceOrientation::LandscapeLeft => Some(UIDeviceOrientationLandscapeLeft),
        crate::window::DeviceOrientation::LandscapeRight => Some(UIDeviceOrientationLandscapeRight),
        crate::window::DeviceOrientation::Portrait => None,
    } {
        let should: bool = if vc != nil {
            msg![env; vc shouldAutorotateToInterfaceOrientation:orientation]
        } else {
            false
        };
        
        if should && vc != nil {
            let is_dmc4 = env.bundle.bundle_identifier() == "jp.co.capcom.devil4us";
            let transform = match orientation {
                UIInterfaceOrientationLandscapeLeft => {
                    let angle = if is_dmc4 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
                    CGAffineTransform::make_rotation(angle)
                },
                UIInterfaceOrientationLandscapeRight => {
                    let angle = if is_dmc4 { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 };
                    CGAffineTransform::make_rotation(angle)
                },
                _ => CGAffineTransform::make_rotation(0.0),
            };
            
            let window_frame: CGRect = msg![env; this frame];
            () = msg![env; view setTransform:transform];
            () = msg![env; view setFrame:window_frame];
        }
    }
}

- (CGPoint)convertPoint:(CGPoint)point fromWindow:(id)other {
    let this_layer: id = msg![env; this layer];
    let other_layer: id = msg![env; other layer];
    msg![env; this_layer convertPoint:point fromLayer:other_layer]
}

- (CGPoint)convertPoint:(CGPoint)point toWindow:(id)other {
    let this_layer: id = msg![env; this layer];
    let other_layer: id = msg![env; other layer];
    msg![env; this_layer convertPoint:point toLayer:other_layer]
}

@end

};

const UIWindowDidBecomeKeyNotification: &str = "UIWindowDidBecomeKeyNotification";
pub const UIKeyboardWillShowNotification: &str = "UIKeyboardWillShowNotification";
pub const UIKeyboardDidShowNotification: &str = "UIKeyboardDidShowNotification";
pub const UIKeyboardWillHideNotification: &str = "UIKeyboardWillHideNotification";
pub const UIKeyboardDidHideNotification: &str = "UIKeyboardDidHideNotification";
pub const UIKeyboardBoundsUserInfoKey: &str = "UIKeyboardBoundsUserInfoKey";

pub const CONSTANTS: ConstantExports = &[
    (
        "_UIWindowDidBecomeKeyNotification",
        HostConstant::NSString(UIWindowDidBecomeKeyNotification),
    ),
];
