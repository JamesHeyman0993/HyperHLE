/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The UIKit framework.

use crate::{msg, msg_class, Environment}; 
use crate::objc::nil;                     
use std::time::Instant;

use crate::dyld::HostConstant;
use crate::mem::{ConstVoidPtr, MutPtr};

pub mod ui_accelerometer;
pub mod ui_action_sheet;
pub mod ui_activity_indicator_view;
pub mod ui_application;
pub mod ui_color;
pub mod ui_device;
pub mod ui_document;
pub mod ui_event;
pub mod ui_font;
pub mod ui_geometry;
pub mod ui_graphics;
pub mod ui_image;
pub mod ui_image_picker_controller;
pub mod ui_keyboard;
pub mod ui_local_notification;
pub mod ui_navigation_bar;
pub mod ui_nib;
pub mod ui_pasteboard;
pub mod ui_pinch_gesture_recognizer;
pub mod ui_popover_controller;
pub mod ui_responder;
pub mod ui_screen;
pub mod ui_screen_mode;
pub mod ui_split_view_controller;
pub mod ui_tab_bar_controller;
pub mod ui_tab_bar_item;
pub mod ui_touch;
pub mod ui_view;
pub mod ui_view_controller;

fn ui_background_task_invalid(env: &mut Environment) -> ConstVoidPtr {
    let ptr: MutPtr<u32> = env.mem.alloc(4).cast();
    env.mem.write(ptr, 0xFFFF_FFFFu32);
    ptr.cast().cast_const()
}

fn ui_window_level_normal(env: &mut Environment) -> ConstVoidPtr {
    let ptr: MutPtr<u32> = env.mem.alloc(4).cast();
    env.mem.write(ptr, 0.0f32.to_bits());
    ptr.cast().cast_const()
}

fn ui_window_level_status_bar(env: &mut Environment) -> ConstVoidPtr {
    let ptr: MutPtr<u32> = env.mem.alloc(4).cast();
    env.mem.write(ptr, 1000.0f32.to_bits());
    ptr.cast().cast_const()
}

fn ui_window_level_alert(env: &mut Environment) -> ConstVoidPtr {
    let ptr: MutPtr<u32> = env.mem.alloc(4).cast();
    env.mem.write(ptr, 2000.0f32.to_bits());
    ptr.cast().cast_const()
}

pub const CONSTANTS: &[(&str, HostConstant)] = &[
    (
        "_UIBackgroundTaskInvalid",
        HostConstant::Custom(ui_background_task_invalid),
    ),
    (
        "_UIImagePickerControllerOriginalImage",
        HostConstant::NSString("UIImagePickerControllerOriginalImage"),
    ),
    (
        "_UIImagePickerControllerEditedImage",
        HostConstant::NSString("UIImagePickerControllerEditedImage"),
    ),
    (
        "_UIImagePickerControllerCropRect",
        HostConstant::NSString("UIImagePickerControllerCropRect"),
    ),
    (
        "_UIImagePickerControllerMediaType",
        HostConstant::NSString("UIImagePickerControllerMediaType"),
    ),
    (
        "_UIImagePickerControllerMediaURL",
        HostConstant::NSString("UIImagePickerControllerMediaURL"),
    ),
    (
        "_UIImagePickerControllerReferenceURL",
        HostConstant::NSString("UIImagePickerControllerReferenceURL"),
    ),
    (
        "_UIScreenDidConnectNotification",
        HostConstant::NSString("UIScreenDidConnectNotification"),
    ),
    (
        "_UIScreenDidDisconnectNotification",
        HostConstant::NSString("UIScreenDidDisconnectNotification"),
    ),
    (
        "_UITrackingRunLoopMode",
        HostConstant::NSString("UITrackingRunLoopMode"),
    ),
    (
        "_UIApplicationLaunchOptionsLocalNotificationKey",
        HostConstant::NSString("UIApplicationLaunchOptionsLocalNotificationKey"),
    ),
    (
        "_UIWindowLevelNormal",
        HostConstant::Custom(ui_window_level_normal),
    ),
    (
        "_UIWindowLevelStatusBar",
        HostConstant::Custom(ui_window_level_status_bar),
    ),
    (
        "_UIWindowLevelAlert",
        HostConstant::Custom(ui_window_level_alert),
    ),
    (
        "_UIApplicationWillChangeStatusBarOrientationNotification",
        HostConstant::NSString("UIApplicationWillChangeStatusBarOrientationNotification"),
    ),
    (
        "_UIApplicationDidChangeStatusBarOrientationNotification",
        HostConstant::NSString("UIApplicationDidChangeStatusBarOrientationNotification"),
    ),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/UIKit.framework/UIKit",
    aliases: &[],
    class_exports: &[
        ui_accelerometer::CLASSES,
        ui_action_sheet::CLASSES,
        ui_activity_indicator_view::CLASSES,
        ui_application::CLASSES,
        ui_color::CLASSES,
        ui_device::CLASSES,
        ui_document::CLASSES,
        ui_event::CLASSES,
        ui_font::CLASSES,
        ui_image::CLASSES,
        ui_image_picker_controller::CLASSES,
        ui_keyboard::CLASSES,
        ui_local_notification::CLASSES,
        ui_navigation_bar::CLASSES,
        ui_nib::CLASSES,
        ui_pasteboard::CLASSES,
        ui_pinch_gesture_recognizer::CLASSES,
        ui_popover_controller::CLASSES,
        ui_responder::CLASSES,
        ui_screen_mode::CLASSES,
        ui_screen::CLASSES,
        ui_split_view_controller::CLASSES,
        ui_tab_bar_item::CLASSES,
        ui_tab_bar_controller::CLASSES,
        ui_touch::CLASSES,
        ui_view::CLASSES,
        ui_view::ui_alert_view::CLASSES,
        ui_view::ui_control::CLASSES,
        ui_view::ui_control::ui_bar_button_item::CLASSES,
        ui_view::ui_control::ui_button::CLASSES,
        ui_view::ui_control::ui_segmented_control::CLASSES,
        ui_view::ui_control::ui_slider::CLASSES,
        ui_view::ui_control::ui_text_field::CLASSES,
        ui_view::ui_control::ui_switch::CLASSES,
        ui_view::ui_image_view::CLASSES,
        ui_view::ui_label::CLASSES,
        ui_view::ui_page_control::CLASSES,
        ui_view::ui_picker_view::CLASSES,
        ui_view::ui_scroll_view::CLASSES,
        ui_view::ui_scroll_view::ui_text_view::CLASSES,
        ui_view::ui_table_view::CLASSES,
        ui_view::ui_text_selection_view::CLASSES,
        ui_view::ui_toolbar::CLASSES,
        ui_view::ui_web_view::CLASSES,
        ui_view::ui_window::CLASSES,
        ui_view_controller::CLASSES,
        ui_view_controller::ui_navigation_controller::CLASSES,
    ],
    constant_exports: &[
        ui_application::CONSTANTS,
        ui_device::CONSTANTS,
        ui_geometry::CONSTANTS,
        ui_keyboard::CONSTANTS,
        ui_local_notification::CONSTANTS,
        ui_screen::CONSTANTS, 
        ui_view::ui_control::ui_text_field::CONSTANTS,
        ui_view::ui_window::CONSTANTS,
        CONSTANTS,
    ],
    function_exports: &[
        ui_application::FUNCTIONS,
        ui_geometry::FUNCTIONS,
        ui_graphics::FUNCTIONS,
        ui_image::FUNCTIONS,
        ui_image_picker_controller::FUNCTIONS,
    ],
};

#[derive(Default)]
pub struct State {
    pub ui_accelerometer: ui_accelerometer::State,
    pub ui_application: ui_application::State,
    pub ui_color: ui_color::State,
    pub ui_device: ui_device::State,
    pub ui_font: ui_font::State,
    pub ui_geometry: ui_geometry::State,
    pub ui_graphics: ui_graphics::State,
    pub ui_image: ui_image::State,
    pub ui_screen: ui_screen::State,
    pub ui_touch: ui_touch::State,
    pub ui_view: ui_view::State,
    pub ui_responder: ui_responder::State,
}

pub fn handle_events(env: &mut Environment) -> Option<Instant> {
    use crate::window::Event;
    use crate::window::TextInputEvent;

    loop {
        let Some(event) = env.window_mut().pop_event() else {
            break;
        };

        match event {
            Event::Quit => {
                echo!("User requested quit, exiting.");
                ui_application::exit(env);
            }
            Event::TouchesDown(..) | Event::TouchesMove(..) | Event::TouchesUp(..) => {
                ui_touch::handle_event(env, event)
            }
            Event::AppWillResignActive => {
                log!("Handling app-will-resign-active event: ignoring to prevent pause.");
                // Use continue to skip this event and move to the next one in the loop
                continue;
                                    }
             
            Event::AppWillTerminate => {
                log!("Handling app-will-terminate event: ignoring.");
            }
            Event::EnterDebugger => {
                if env.is_debugging_enabled() {
                    log!("Handling EnterDebugger event: entering debugger.");
                    env.enter_debugger(/* reason: */ None);
                } else {
                    log!("Ignoring EnterDebugger event: no debugger connected.");
                }
            }
            Event::TextInput(text_event) => {
                let responder = env.framework_state.uikit.ui_responder.first_responder;
                let class = msg![env; responder class];
                let ui_text_field_class = env.objc.get_known_class("UITextField", &mut env.mem);

                if !responder.is_null() && env.objc.class_is_subclass_of(class, ui_text_field_class)
                {
                    match text_event {
                        TextInputEvent::Text(text) => {
                            ui_view::ui_control::ui_text_field::handle_text(env, responder, text)
                        }
                        TextInputEvent::Backspace => {
                            ui_view::ui_control::ui_text_field::handle_backspace(env, responder)
                        }
                        TextInputEvent::Return => {
                            ui_view::ui_control::ui_text_field::handle_return(env, responder)
                        }
                    }
                }
            }
        }
    }

    ui_accelerometer::handle_accelerometer(env)
}
