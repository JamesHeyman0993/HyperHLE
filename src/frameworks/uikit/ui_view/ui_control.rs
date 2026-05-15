/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIControl`.
//!
//! Useful resources:
//! - The [Target-Action section](https://developer.apple.com/library/archive/documentation/General/Conceptual/CocoaEncyclopedia/Target-Action/Target-Action.html) of Apple's "Concepts in Objective-C Programming".

pub mod ui_bar_button_item;
pub mod ui_button;
pub mod ui_segmented_control;
pub mod ui_slider;
pub mod ui_switch;
pub mod ui_text_field;

use crate::frameworks::core_graphics::CGPoint;
use crate::frameworks::foundation::NSUInteger;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_send, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr, SEL, msg_class,
};
use crate::Environment;

// TODO: There are many members of this enum missing.
pub type UIControlEvents = NSUInteger;
const UIControlEventTouchDown: UIControlEvents = 1 << 0;
const UIControlEventTouchDragInside: UIControlEvents = 1 << 2;
const UIControlEventTouchDragOutside: UIControlEvents = 1 << 3;
const UIControlEventTouchDragEnter: UIControlEvents = 1 << 4;
const UIControlEventTouchDragExit: UIControlEvents = 1 << 5;
pub const UIControlEventTouchUpInside: UIControlEvents = 1 << 6;
const UIControlEventTouchUpOutside: UIControlEvents = 1 << 7;
pub const UIControlEventValueChanged: UIControlEvents = 1 << 12;

struct UIControlHostObject {
    superclass: super::UIViewHostObject,
    enabled: bool,
    selected: bool,
    highlighted: bool,
    /// `UITouch*` of the touch currently being tracked, [nil] if none
    tracked_touch: id,
    tracking: bool,
    /// See `addTarget:action:forControlEvents:`. The target is a weak
    /// reference!
    action_targets: Vec<(id, SEL, UIControlEvents)>,
}
impl_HostObject_with_superclass!(UIControlHostObject);
impl Default for UIControlHostObject {
    fn default() -> Self {
        UIControlHostObject {
            superclass: Default::default(),
            enabled: true,
            selected: false,
            highlighted: false,
            tracked_touch: nil,
            tracking: false,
            action_targets: Vec::new(),
        }
    }
}

type UIControlState = NSUInteger;
pub const UIControlStateNormal: UIControlState = 0;
const UIControlStateHighlighted: UIControlState = 1 << 0;
const UIControlStateDisabled: UIControlState = 1 << 1;
const UIControlStateSelected: UIControlState = 1 << 2;
#[allow(dead_code)]
const UIControlStateFocused: UIControlState = 1 << 3;

fn send_actions(env: &mut Environment, this: id, event: id, control_event: UIControlEvents) {
    log_dbg!(
        "Control event {:?} in control {:?} for event {:?}",
        control_event,
        this,
        event,
    );

    let UIControlHostObject { action_targets, .. } = env.objc.borrow(this);
    let action_targets: Vec<_> = action_targets
        .iter()
        .filter(|&(_target, _action, for_control_events)| (for_control_events & control_event) != 0)
        .map(|&(target, action, _for_control_events)| (target, action))
        .collect();

    for (target, action) in action_targets {
        // We no longer assert target != nil here because sendAction now handles redirection
        () = msg![env; this sendAction:action to:target forEvent:event];
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// abstract class
@implementation UIControl: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIControlHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    let UIControlHostObject {
        superclass: _,
        enabled: _,
        selected: _,
        highlighted: _,
        tracking: _,
        action_targets: _, 
        tracked_touch,
    } = std::mem::take(env.objc.borrow_mut(this));

    release(env, tracked_touch);

    msg_super![env; this dealloc]
}

- (UIControlState)state {
    let &UIControlHostObject {
        highlighted,
        enabled,
        selected,
        ..
    } = env.objc.borrow(this);
    let mut state = 0; 
    if highlighted {
        state |= UIControlStateHighlighted;
    }
    if !enabled {
        state |= UIControlStateDisabled;
    }
    if selected {
        state |= UIControlStateSelected;
    }
    state
}

- (bool)isEnabled {
    env.objc.borrow::<UIControlHostObject>(this).enabled
}
- (())setEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIControlHostObject>(this).enabled = enabled;
}

- (bool)isSelected {
    env.objc.borrow::<UIControlHostObject>(this).selected
}
- (())setSelected:(bool)selected {
    env.objc.borrow_mut::<UIControlHostObject>(this).selected = selected;
}

- (bool)isHighlighted {
    env.objc.borrow::<UIControlHostObject>(this).highlighted
}
- (())setHighlighted:(bool)highlighted {
    env.objc.borrow_mut::<UIControlHostObject>(this).highlighted = highlighted;
}

- (bool)tracking {
    env.objc.borrow::<UIControlHostObject>(this).tracking
}

- (())cancelTrackingWithEvent:(id)_event { }

- (bool)beginTrackingWithTouch:(id)_touch withEvent:(id)_event { true }
- (bool)continueTrackingWithTouch:(id)_touch withEvent:(id)_event { true }
- (())endTrackingWithTouch:(id)_touch withEvent:(id)_event {
    env.objc.borrow_mut::<UIControlHostObject>(this).tracking = false;
}

- (())touchesBegan:(id)touches withEvent:(id)event {
    if !msg![env; this isEnabled] { return; }

    let touch: id = msg![env; touches anyObject];
    if !msg![env; this beginTrackingWithTouch:touch withEvent:event] { return; }

    retain(env, touch);
    let host_obj = env.objc.borrow_mut::<UIControlHostObject>(this);
    host_obj.tracking = true;
    let old_touch = std::mem::replace(&mut host_obj.tracked_touch, touch);
    release(env, old_touch);
    
    () = msg![env; this setHighlighted:true];
    send_actions(env, this, event, UIControlEventTouchDown);
}

- (())touchesMoved:(id)touches withEvent:(id)event {
    if !msg![env; this isEnabled] { return; }

    let touch: id = msg![env; touches anyObject];
    let tracked_touch = env.objc.borrow::<UIControlHostObject>(this).tracked_touch;
    if tracked_touch != touch { return; }

    if !msg![env; this continueTrackingWithTouch:touch withEvent:event] {
        release(env, tracked_touch);
        env.objc.borrow_mut::<UIControlHostObject>(this).tracked_touch = nil;
        env.objc.borrow_mut::<UIControlHostObject>(this).tracking = false;
        () = msg![env; this setHighlighted:false];
    }

    let old_pos: CGPoint = msg![env; touch previousLocationInView:this];
    let new_pos: CGPoint = msg![env; touch locationInView:this];
    let was_inside = msg![env; this pointInside:old_pos withEvent:event];
    let is_inside = msg![env; this pointInside:new_pos withEvent:event];

    send_actions(env, this, event, match (was_inside, is_inside) {
        (true, true) => UIControlEventTouchDragInside,
        (false, false) => UIControlEventTouchDragOutside,
        (false, true) => UIControlEventTouchDragEnter,
        (true, false) => UIControlEventTouchDragExit,
    });
}

- (())touchesEnded:(id)touches withEvent:(id)event {
    let touch: id = msg![env; touches anyObject];
    let tracked_touch = env.objc.borrow::<UIControlHostObject>(this).tracked_touch;
    if tracked_touch != touch { return; }

    () = msg![env; this endTrackingWithTouch:touch withEvent:event];
    release(env, tracked_touch);
    env.objc.borrow_mut::<UIControlHostObject>(this).tracked_touch = nil;
    () = msg![env; this setHighlighted:false];

    let new_pos: CGPoint = msg![env; touch locationInView:this];
    let is_inside = msg![env; this pointInside:new_pos withEvent:event];

    send_actions(env, this, event, match is_inside {
        true => UIControlEventTouchUpInside,
        false => UIControlEventTouchUpOutside,
    });
}

- (())addTarget:(id)target action:(SEL)action forControlEvents:(UIControlEvents)events {
    // If target is nil, we don't return anymore. We store it as nil so sendAction can redirect it later.
    if target == nil {
        log_dbg!("HyperHLE: UIButton addTarget with nil target for action {:?}", action);
    }

    let sel_str = action.as_str(&env.mem);
    let colon_count = sel_str.bytes().filter(|&b| b == b':').count();
    assert!([0, 1, 2].contains(&colon_count));

    env.objc.borrow_mut::<UIControlHostObject>(this).action_targets.push((target, action, events));
}

- (())sendAction:(SEL)action to:(id)target forEvent:(id)event {
    let mut actual_target = target;

    // --- START REDIRECTION FIX ---
    if actual_target == nil {
        // In your project, msg_class! calls a method on the class directly.
        // We call 'sharedApplication' on the 'UIApplication' class here.
        let shared_app: id = msg_class![env; UIApplication sharedApplication];
        
        if shared_app != nil {
            // Now we get the delegate from that instance
            actual_target = msg![env; shared_app delegate];
            
            if actual_target != nil {
                log::info!("HyperHLE: Redirecting nil target to App Delegate: {:?}", actual_target);
            }
        }
    }

    if actual_target == nil {
        log::warn!("HyperHLE: No target found for action {:?}", action);
        return;
    }
    // --- END REDIRECTION FIX ---

    let sel_str = action.as_str(&env.mem);
    let colon_count = sel_str.bytes().filter(|&b| b == b':').count();
    match colon_count {
        0 => {
            log_dbg!("Sending {:?} message to {:?} (no args)", action, actual_target);
            () = msg_send(env, (actual_target, action));
        }
        1 => {
            log_dbg!("Sending {:?} message to {:?} (one arg)", action, actual_target);
            () = msg_send(env, (actual_target, action, this));
        }
        2 => {
            log_dbg!("Sending {:?} message to {:?} (two args)", action, actual_target);
            () = msg_send(env, (actual_target, action, this, event));
        }
        _ => panic!(),
    };
}
    
@end

};
