/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIWindow`.

use super::UIViewHostObject;
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_graphics::cg_affine_transform::{
    CGAffineTransform, CGAffineTransformIdentity,
};
use crate::frameworks::core_graphics::{CGPoint, CGRect};
use crate::frameworks::foundation::ns_string;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientationLandscapeLeft, UIInterfaceOrientationLandscapeRight,
};
use crate::frameworks::uikit::ui_device::{
    UIDeviceOrientationLandscapeLeft, UIDeviceOrientationLandscapeRight,
};
use crate::objc::{id, msg, msg_class, msg_super, nil, objc_classes, release, retain, ClassExports};
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub windows: Vec<id>,
    pub key_window: Option<id>,
    pub root_view_controllers: HashMap<id, id>,
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIWindow: UIView

- (id)initWithFrame:(CGRect)frame {
    let this = msg_super![env; this initWithFrame:frame];
    () = msg_super![env; this setHidden:true];

    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    list.push(this);
    this
}

- (id)initWithCoder:(id)coder {
    let this = msg_super![env; this initWithCoder:coder];
    () = msg_super![env; this setHidden:true];

    let screen: id = msg_class![env; UIScreen mainScreen];
    let screen_bounds: CGRect = msg![env; screen bounds];
    let current_bounds: CGRect = msg![env; this bounds];
    if current_bounds.size != screen_bounds.size {
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
    if let Some(root_vc) = env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .root_view_controllers
        .remove(&this)
    {
        release(env, root_vc);
    }
    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    if let Some(idx) = list.iter().position(|&w| w == this) {
        list.remove(idx);
    }
    msg_super![env; this dealloc]
}

- (())layoutIfNeeded {
    () = msg![env; this layoutSubviews];
}

- (id)hitTest:(CGPoint)point withEvent:(id)event {
    // Permissive layout traversal ensures touches never get discarded by screen scaling anomalies
    let subviews = env.objc.borrow::<super::UIViewHostObject>(this).subviews.clone();
    for subview in subviews.into_iter().rev() {
        let hidden: bool = msg![env; subview isHidden];
        let alpha: crate::frameworks::core_graphics::CGFloat = msg![env; subview alpha];
        let interactible: bool = msg![env; subview isUserInteractionEnabled];
        if hidden || alpha < 0.01 || !interactible { continue; }
        let sub_point: CGPoint = msg![env; subview convertPoint:point fromView:this];
        let hit: id = msg![env; subview hitTest:sub_point withEvent:event];
        if hit != nil { return hit; }
    }
    this
}

- (())setHidden:(bool)is_hidden {
    () = msg_super![env; this setHidden:is_hidden];
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
    let previous = env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .root_view_controllers
        .get(&this)
        .copied()
        .unwrap_or(nil);

    if previous == view_controller {
        return;
    }

    if view_controller != nil {
        retain(env, view_controller);
    }

    if previous != nil {
        let previous_view: id = msg![env; previous view];
        if previous_view != nil {
            () = msg![env; previous_view removeFromSuperview];
        }
        release(env, previous);
    }

    if view_controller != nil {
        let view: id = msg![env; view_controller view];
        let bounds: CGRect = msg![env; this bounds];
        () = msg![env; view setFrame:bounds];
        () = msg![env; this addSubview:view];
        env.framework_state
            .uikit
            .ui_view
            .ui_window
            .root_view_controllers
            .insert(this, view_controller);
    } else {
        env.framework_state
            .uikit
            .ui_view
            .ui_window
            .root_view_controllers
            .remove(&this);
    }
}

- (id)rootViewController {
    env.framework_state
        .uikit
        .ui_view
        .ui_window
        .root_view_controllers
        .get(&this)
        .copied()
        .unwrap_or(nil)
}

- (id)nextResponder {
    msg_class![env; UIApplication sharedApplication]
}

- (())addSubview:(id)view {
    if view == nil || env.objc.borrow::<UIViewHostObject>(view).view_controller == nil {
        () = msg_super![env; this addSubview:view];
        return;
    }

    let was_subview = env
        .objc
        .borrow::<UIViewHostObject>(this)
        .subviews
        .contains(&view);
    let view_hidden: bool = msg![env; view isHidden];
    let should_fire_appearance = !view_hidden;

    let vc = env.objc.borrow::<UIViewHostObject>(view).view_controller;
    if should_fire_appearance {
        () = msg![env; vc viewWillAppear:false];
    }
    () = msg_super![env; this addSubview:view];
    if should_fire_appearance {
        () = msg![env; vc viewDidAppear:false];
    }

    if let Some(orientation) = match env.window.as_ref().unwrap().current_rotation() {
        crate::window::DeviceOrientation::LandscapeLeft => Some(UIDeviceOrientationLandscapeLeft),
        crate::window::DeviceOrientation::LandscapeRight => Some(UIDeviceOrientationLandscapeRight),
        crate::window::DeviceOrientation::Portrait => None,
    } {
        let should = msg![env; vc shouldAutorotateToInterfaceOrientation:orientation];
        if should {
            let transform = match orientation {
                UIInterfaceOrientationLandscapeLeft => CGAffineTransform::make_rotation(-std::f32::consts::FRAC_PI_2),
                UIInterfaceOrientationLandscapeRight => CGAffineTransform::make_rotation(std::f32::consts::FRAC_PI_2),
                _ => CGAffineTransformIdentity
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
const UIWindowDidResignKeyNotification: &str = "UIWindowDidResignKeyNotification";
const UIWindowDidBecomeHiddenNotification: &str = "UIWindowDidBecomeHiddenNotification";
const UIWindowDidBecomeVisibleNotification: &str = "UIWindowDidBecomeVisibleNotification";

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
    (
        "_UIWindowDidResignKeyNotification",
        HostConstant::NSString(UIWindowDidResignKeyNotification),
    ),
    (
        "_UIWindowDidBecomeHiddenNotification",
        HostConstant::NSString(UIWindowDidBecomeHiddenNotification),
    ),
    (
        "_UIWindowDidBecomeVisibleNotification",
        HostConstant::NSString(UIWindowDidBecomeVisibleNotification),
    ),
];
