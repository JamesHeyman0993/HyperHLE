/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIScreen`.

use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, TrivialHostObject, SEL};
use crate::dyld::{ConstantExports, HostConstant}; // Added imports

#[derive(Default)]
pub struct State {
    main_screen: Option<id>,
}

// Added to fix _UIScreenDidConnectNotification crash
pub const CONSTANTS: ConstantExports = &[
    ("_UIScreenDidConnectNotification", HostConstant::NSString("UIScreenDidConnectNotification")),
    ("_UIScreenDidDisconnectNotification", HostConstant::NSString("UIScreenDidDisconnectNotification")),
    ("_UIScreenModeDidChangeNotification", HostConstant::NSString("UIScreenModeDidChangeNotification")),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIScreen: NSObject

// MARK: - Singleton

+ (id)mainScreen {
    if let Some(screen) = env.framework_state.uikit.ui_screen.main_screen {
        screen
    } else {
        let new = env.objc.alloc_static_object(
            this,
            Box::new(TrivialHostObject),
            &mut env.mem,
        );
        env.framework_state.uikit.ui_screen.main_screen = Some(new);
        new
    }
}

+ (id)screens {
    // Only one screen is ever available.
    let main: id = msg![env; this mainScreen];
    msg_class![env; NSArray arrayWithObject:main]
}

// MARK: - Retain / release (singleton — no-ops)

- (id)retain      { this }
- (())release     {}
- (id)autorelease { this }

// MARK: - Geometry

- (CGRect)bounds {
    let (width, height) = env.window().device_family().portrait_size();
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width:  width  as CGFloat,
            height: height as CGFloat,
        },
    }
}

- (CGRect)nativeBounds {
    let scale: CGFloat = msg![env; this scale];
    let bounds: CGRect = msg![env; this bounds];
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width:  bounds.size.width * scale,
            height: bounds.size.height * scale,
        },
    }
}
    
- (CGRect)applicationFrame {
    let mut bounds: CGRect = msg![env; this bounds];
    const STATUS_BAR_HEIGHT: CGFloat = 20.0;
    if !env.framework_state.uikit.ui_application.status_bar_hidden {
        bounds.origin.y    += STATUS_BAR_HEIGHT;
        bounds.size.height -= STATUS_BAR_HEIGHT;
    }
    bounds
}

// MARK: - Scale

- (CGFloat)scale {
    env.window().device_family().scale_factor() as CGFloat
}

- (CGFloat)nativeScale {
    msg![env; this scale]
}
    
// MARK: - Brightness

- (CGFloat)brightness {
    1.0
}

- (())setBrightness:(CGFloat)_brightness {
    log!("TODO: [UIScreen setBrightness:] (not implemented)");
}

- (bool)wantsSoftwareDimming {
    false
}

- (())setWantsSoftwareDimming:(bool)_value {
    log!("TODO: [UIScreen setWantsSoftwareDimming:] (not implemented)");
}

// MARK: - Display mode / overscan

- (id)currentMode {
    let (width, height) = env.window().device_family().portrait_size();
    let size = CGSize {
        width:  width  as CGFloat,
        height: height as CGFloat,
    };
    crate::frameworks::uikit::ui_screen_mode::from_size(env, size, 1.0)
}

- (id)preferredMode {
    nil
}

- (id)availableModes {
    let current: id = msg![env; this currentMode];
    // Return an array containing the current mode
    msg_class![env; NSArray arrayWithObject:current]
}
    
- (CGFloat)overscanCompensationInsets {
    0.0
}

// MARK: - Mirroring (iOS 4.3+)

- (id)mirroredScreen {
    nil
}

- (bool)isCaptured {
    false
}

// MARK: - Coordinate conversion helpers

- (CGRect)convertRect:(CGRect)rect toScreen:(id)_other_screen {
    rect
}

- (CGRect)convertRect:(CGRect)rect fromScreen:(id)_other_screen {
    rect
}

- (CGPoint)convertPoint:(CGPoint)point toScreen:(id)_other_screen {
    point
}

- (CGPoint)convertPoint:(CGPoint)point fromScreen:(id)_other_screen {
    point
}

// MARK: - Display Link

- (id)displayLinkWithTarget:(id)target selector:(SEL)selector {
    msg_class![env; CADisplayLink displayLinkWithTarget:target selector:selector]
}

@end

};
