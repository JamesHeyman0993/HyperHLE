/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIPinchGestureRecognizer`.

use crate::frameworks::core_graphics::CGFloat;
use crate::frameworks::foundation::NSInteger;
use crate::objc::{id, nil, objc_classes, ClassExports, HostObject, NSZonePtr, SEL};

// Честные константы состояний жеста (по документации Apple)
pub type UIGestureRecognizerState = NSInteger;
pub const UIGestureRecognizerStatePossible: UIGestureRecognizerState = 0;
pub const UIGestureRecognizerStateBegan: UIGestureRecognizerState = 1;
pub const UIGestureRecognizerStateChanged: UIGestureRecognizerState = 2;
pub const UIGestureRecognizerStateEnded: UIGestureRecognizerState = 3;
pub const UIGestureRecognizerStateCancelled: UIGestureRecognizerState = 4;
pub const UIGestureRecognizerStateFailed: UIGestureRecognizerState = 5;
pub const UIGestureRecognizerStateRecognized: UIGestureRecognizerState =
    UIGestureRecognizerStateEnded;

// MARK: - UIGestureRecognizer host object
struct UIGestureRecognizerHostObject {
    state: UIGestureRecognizerState,
    view: id,
    enabled: bool,
    cancels_touches_in_view: bool,
    delays_touches_began: bool,
    delays_touches_ended: bool,
    delegate: id,
}
impl HostObject for UIGestureRecognizerHostObject {}

// MARK: - UIPinchGestureRecognizer host object
struct UIPinchGestureRecognizerHostObject {
    scale: CGFloat,
    velocity: CGFloat,
}
impl HostObject for UIPinchGestureRecognizerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// =========================================================================
// MARK: - UIGestureRecognizer (Базовый класс)
// =========================================================================

@implementation UIGestureRecognizer: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIGestureRecognizerHostObject {
        state: UIGestureRecognizerStatePossible,
        view: nil, // Будет назначен, когда жест добавят на UIView
        enabled: true,
        cancels_touches_in_view: true, // В iOS по умолчанию true
        delays_touches_began: false,   // В iOS по умолчанию false
        delays_touches_ended: true,    // В iOS по умолчанию true
        delegate: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// Честный инициализатор
- (id)initWithTarget:(id)_target action:(SEL)_action {
    // В идеале тут нужно сохранять target и action в вектор внутри HostObject,
    // но для старта и прохождения линковки достаточно вернуть self.
    this
}

- (UIGestureRecognizerState)state {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).state
}

- (id)view {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).view
}

- (bool)isEnabled {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).enabled
}

- (())setEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).enabled = enabled;
}

- (bool)cancelsTouchesInView {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).cancels_touches_in_view
}

- (())setCancelsTouchesInView:(bool)value {
    env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).cancels_touches_in_view = value;
}

- (bool)delaysTouchesBegan {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).delays_touches_began
}

- (())setDelaysTouchesBegan:(bool)value {
    env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).delays_touches_began = value;
}

- (bool)delaysTouchesEnded {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).delays_touches_ended
}

- (())setDelaysTouchesEnded:(bool)value {
    env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).delays_touches_ended = value;
}

- (id)delegate {
    env.objc.borrow::<UIGestureRecognizerHostObject>(this).delegate
}

- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).delegate = delegate;
}

@end

// =========================================================================
// MARK: - UIPinchGestureRecognizer
// =========================================================================

@implementation UIPinchGestureRecognizer: UIGestureRecognizer

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIPinchGestureRecognizerHostObject {
        scale: 1.0,
        velocity: 0.0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// MARK: - Свойства UIPinchGestureRecognizer

- (CGFloat)scale {
    env.objc.borrow::<UIPinchGestureRecognizerHostObject>(this).scale
}

- ((()))setScale:(CGFloat)scale {
    env.objc.borrow_mut::<UIPinchGestureRecognizerHostObject>(this).scale = scale;
}

- (((CGFloat)))velocity {
    env.objc.borrow::<UIPinchGestureRecognizerHostObject>(this).velocity
}

@end

// =========================================================================
// MARK: - UISwipeGestureRecognizer & UIRotationGestureRecognizer Stubs
// =========================================================================

@implementation UISwipeGestureRecognizer: UIGestureRecognizer
// Automatically inherits allocWithZone: and initWithTarget:action: from UIGestureRecognizer
@end

@implementation UIRotationGestureRecognizer: UIGestureRecognizer
// Automatically inherits allocWithZone: and initWithTarget:action: from UIGestureRecognizer
@end

};
