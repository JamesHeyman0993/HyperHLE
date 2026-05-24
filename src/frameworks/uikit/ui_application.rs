/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIApplication` and `UIApplicationMain`.

use super::ui_device::*;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str};
use crate::frameworks::foundation::{ns_array, ns_string, NSInteger, NSUInteger};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};
use crate::window::DeviceOrientation;
use crate::Environment;

#[derive(Default)]
pub struct State {
    /// [UIApplication sharedApplication]
    shared_application: Option<id>,
    pub(super) status_bar_hidden: bool,
    /// Whether shake to edit is enabled
    pub(super) application_supports_shake_to_edit: bool,
    pub(super) ignoring_interaction_events_count: u32,
}

struct UIApplicationHostObject {
    delegate: id,
    delegate_is_retained: bool,
    status_bar_style: UIStatusBarStyle,
    application_icon_badge_number: NSInteger,
}
impl HostObject for UIApplicationHostObject {}

pub type UIInterfaceOrientation = UIDeviceOrientation;
#[allow(unused)]
pub const UIInterfaceOrientationPortrait: UIInterfaceOrientation = UIDeviceOrientationPortrait;
#[allow(unused)]
pub const UIInterfaceOrientationPortraitUpsideDown: UIInterfaceOrientation =
    UIDeviceOrientationPortraitUpsideDown;
pub const UIInterfaceOrientationLandscapeLeft: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeRight;
pub const UIInterfaceOrientationLandscapeRight: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeLeft;
type UIRemoteNotificationType = NSUInteger;
type UIStatusBarAnimation = NSInteger;
type UIStatusBarStyle = NSInteger;
pub type UIApplicationState = NSInteger;
pub const UIApplicationStateActive: UIApplicationState = 0;
pub const UIApplicationStateInactive: UIApplicationState = 1;
pub const UIApplicationStateBackground: UIApplicationState = 2;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);
@implementation UIApplication: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIApplicationHostObject {
        delegate: nil,
        delegate_is_retained: false,
        status_bar_style: 0,
        application_icon_badge_number: 0,
    });
    env.objc.alloc_static_object(this, host_object, &mut env.mem)
}

+ (id)sharedApplication {
    env.framework_state.uikit.ui_application.shared_application.unwrap_or(nil)
}

- (())setNetworkActivityIndicatorVisible:(bool)visible {
    log_dbg!("Stubbed setNetworkActivityIndicatorVisible: {}", visible);
}

- (bool)isNetworkActivityIndicatorVisible {
    false
}

- (id)init {
    assert!(env.framework_state.uikit.ui_application.shared_application.is_none());
    env.framework_state.uikit.ui_application.shared_application = Some(this);
    this
}

- (id)retain { this }
- (id)autorelease { this }
- (())release {}

- (id)delegate {
    env.objc.borrow::<UIApplicationHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    let host_object = env.objc.borrow_mut::<UIApplicationHostObject>(this);
    let old_delegate = std::mem::replace(&mut host_object.delegate, delegate);
    if host_object.delegate_is_retained {
        host_object.delegate_is_retained = false;
        if delegate != old_delegate {
            release(env, old_delegate);
        }
    }
}

- (bool)isStatusBarHidden {
    env.framework_state.uikit.ui_application.status_bar_hidden
}
- (())setStatusBarHidden:(bool)hidden {
    env.framework_state.uikit.ui_application.status_bar_hidden = hidden;
}
- (())setStatusBarHidden:(bool)hidden animated:(bool)_animated {
    () = msg![env; this setStatusBarHidden:hidden];
}
- (())setStatusBarHidden:(bool)hidden withAnimation:(UIStatusBarAnimation)_animation {
    () = msg![env; this setStatusBarHidden:hidden];
}

- (())setStatusBarStyle:(UIStatusBarStyle)style {
    env.objc.borrow_mut::<UIApplicationHostObject>(this).status_bar_style = style;
}
- (UIStatusBarStyle)statusBarStyle {
    env.objc.borrow::<UIApplicationHostObject>(this).status_bar_style
}
- (())setStatusBarStyle:(UIStatusBarStyle)style animated:(bool)_animated {
    () = msg![env; this setStatusBarStyle:style];
}

- (UIInterfaceOrientation)statusBarOrientation {
    // Preserve layout force for landscape locked titles
    if !env.bundle.is_null() {
        let bundle = env.bundle.as_ref();
        let bundle_id = bundle.bundle_identifier();
        if bundle_id == "com.iplay.ff63d" || bundle_id == "com.saban.powerrangersbash" {
            return UIInterfaceOrientationLandscapeRight;
        }
    }
     
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIDeviceOrientationPortrait,
        DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight
    }
}

- (f64)statusBarOrientationAnimationDuration {
    0.3
}

- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation {
    match orientation {
        UIDeviceOrientationUnknown => {}
        UIDeviceOrientationPortrait => {
            env.on_parent_stack_in_coroutine(|window, _| window.rotate_device(DeviceOrientation::Portrait));
        }
        UIDeviceOrientationLandscapeLeft => {
            env.on_parent_stack_in_coroutine(|window, _| window.rotate_device(DeviceOrientation::LandscapeLeft));
        }
        UIDeviceOrientationLandscapeRight => {
            env.on_parent_stack_in_coroutine(|window, _| window.rotate_device(DeviceOrientation::LandscapeRight));
        }
        _ => {
            log!("Warning: Orientation {} not handled yet (ignoring to prevent panic)", orientation);
        }
    }
}

- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation animated:(bool)_animated {
    () = msg![env; this setStatusBarOrientation:orientation];
}

- (bool)isIdleTimerDisabled {
    !env.window().is_screen_saver_enabled()
}
- (())setIdleTimerDisabled:(bool)disabled {
    env.on_parent_stack_in_coroutine(|window, _| window.set_screen_saver_enabled(!disabled));
}

- (bool)canOpenURL:(id)url {
    if url == nil { return false; }
    let ns_string: id = msg![env; url scheme];
    if ns_string == nil { return false; }
    let scheme = ns_string::to_rust_string(env, ns_string);
    let scheme_lower = scheme.to_lowercase();

    const HOST_HANDLED_SCHEMES: &[&str] = &[
        "http", "https", "ftp", "tel", "telprompt", "facetime", "facetime-audio",
        "mailto", "sms", "imessage", "file", "data", "itms", "itms-apps",
        "itms-services", "itmss", "maps",
    ];
    if HOST_HANDLED_SCHEMES.contains(&scheme_lower.as_str()) {
        return true;
    }

    let main_bundle: id = msg_class![env; NSBundle mainBundle];
    if main_bundle != nil {
        let key_str = ns_string::get_static_str(env, "LSApplicationQueriesSchemes");
        let allowed_arr: id = msg![env; main_bundle objectForInfoDictionaryKey:key_str];
        if allowed_arr != nil {
            let count: u32 = msg![env; allowed_arr count];
            for i in 0..count {
                let entry: id = msg![env; allowed_arr objectAtIndex:i];
                if entry == nil { continue; }
                let entry_str = ns_string::to_rust_string(env, entry);
                if entry_str.to_lowercase() == scheme_lower {
                    return false;
                }
            }
        }
    }
    false
}

- (bool)openURL:(id)url {
    let ns_string = msg![env; url absoluteString];
    let url_string = ns_string::to_rust_string(env, ns_string);
    if let Err(e) = crate::window::open_url(env, &url_string) {
        echo!("App opened URL {:?} unsuccessfully ({})", url_string, e);
    } else {
        echo!("App opened URL {:?}", url_string);
    }
    log!("UIApplication openURL: ignoring forced exit for compatibility");
    true
}

- (())beginIgnoringInteractionEvents {
    env.framework_state.uikit.ui_application.ignoring_interaction_events_count += 1;
}

- (bool)isIgnoringInteractionEvents {
    env.framework_state.uikit.ui_application.ignoring_interaction_events_count > 0
}

- (())endIgnoringInteractionEvents {
    let count = &mut env.framework_state.uikit.ui_application.ignoring_interaction_events_count;
    if *count > 0 {
        *count -= 1;
    } else {
        log!("Warning: endIgnoringInteractionEvents called without matching beginIgnoringInteractionEvents");
    }
}

- (())sendEvent:(id)event {
    log_dbg!("UIApplication sendEvent: forwarding to key window");
    let window: id = msg![env; this keyWindow];
    if window != nil {
        () = msg![env; window sendEvent:event];
    }
}

- (bool)sendAction:(SEL)action to:(id)target from:(id)sender forEvent:(id)event {
    if target != nil {
        let responds: bool = msg![env; target respondsToSelector:action];
        if responds {
            () = msg![env; target performSelector:action withObject:sender];
            return true;
        }
        return false;
    }
    let mut responder: id = sender;
    while responder != nil {
        let responds: bool = msg![env; responder respondsToSelector:action];
        if responds {
            () = msg![env; responder performSelector:action withObject:sender];
            return true;
        }
        responder = msg![env; responder nextResponder];
    }
    false
}

- (())beginBackgroundTaskWithExpirationHandler:(id)_handler {
    log!("UIApplication beginBackgroundTaskWithExpirationHandler: stubbed");
}

- (())endBackgroundTask:(NSUInteger)_task {
    log!("UIApplication endBackgroundTask: stubbed");
}

- (NSUInteger)backgroundTimeRemaining {
    NSUInteger::MAX
}

- (UIApplicationState)applicationState {
    UIApplicationStateActive
}

- (bool)isProtectedDataAvailable {
    true
}

- (())setMinimumBackgroundFetchInterval:(f64)_interval {
    log!("UIApplication setMinimumBackgroundFetchInterval: stubbed");
}

- (())registerForRemoteNotificationTypes:(UIRemoteNotificationType)types {
    log!("Intercepted registerForRemoteNotificationTypes: {}. Simulating safe environment.", types);
    let delegate: id = msg![env; this delegate];
    if delegate != nil {
        if env.objc.object_has_method_named(&env.mem, delegate, "application:didFailToRegisterForRemoteNotificationsWithError:") {
            () = msg![env; delegate application:this didFailToRegisterForRemoteNotificationsWithError:nil];
        }
    }
}

- ((()))unregisterForRemoteNotifications {
    log!("UIApplication unregisterForRemoteNotifications: stubbed");
}

- (bool)isRegisteredForRemoteNotifications {
    false
}

- (())registerUserNotificationSettings:(id)_settings {
    log!("UIApplication registerUserNotificationSettings: stubbed");
}

- (id)currentUserNotificationSettings {
    nil
}

- (())cancelAllLocalNotifications {
    log_dbg!("UIApplication cancelAllLocalNotifications: stubbed");
}

- (())cancelLocalNotification:(id)_notification {
    log_dbg!("UIApplication cancelLocalNotification: stubbed");
}

- (())scheduleLocalNotification:(id)_notification {
    log_dbg!("UIApplication scheduleLocalNotification: stubbed");
}

- (id)scheduledLocalNotifications {
    msg_class![env; NSArray new]
}

- (())setScheduledLocalNotifications:(id)_notifications {
    log!("UIApplication setScheduledLocalNotifications: stubbed");
}

- (bool)supportsShakeToEdit {
    false
}

- (())setSupportsShakeToEdit:(bool)_value {}

- (bool)applicationSupportsShakeToEdit {
    env.framework_state.uikit.ui_application.application_supports_shake_to_edit
}

- (())setApplicationSupportsShakeToEdit:(bool)value {
    env.framework_state.uikit.ui_application.application_supports_shake_to_edit = value;
}

- (())clearKeychainIfNecessary {}

- (CGRect)statusBarFrame {
    CGRect {
        origin: crate::frameworks::core_graphics::CGPoint { x: 0.0, y: 0.0 },
        size: crate::frameworks::core_graphics::CGSize { width: 320.0, height: 0.0 },
    }
}

- (())presentLocalNotificationNow:(id)_notification {
    log_dbg!("UIApplication presentLocalNotificationNow: stubbed");
}

- (id)keyWindow {
    let Some(key_window) = env.framework_state.uikit.ui_view.ui_window.key_window else {
        return nil;
    };
    key_window
}

- (id)windows {
    let windows: Vec<id> = (*env.framework_state.uikit.ui_view.ui_window.windows).to_vec();
    for window in &windows {
        retain(env, *window);
    }
    let windows = ns_array::from_vec(env, windows);
    autorelease(env, windows)
}

- (UIRemoteNotificationType)enabledRemoteNotificationTypes {
    0
}

- (NSInteger)applicationIconBadgeNumber {
    env.objc.borrow::<UIApplicationHostObject>(this).application_icon_badge_number
}
- (())setApplicationIconBadgeNumber:(NSInteger)bn {
    log_dbg!("setApplicationIconBadgeNumber:{}", bn);
    env.objc.borrow_mut::<UIApplicationHostObject>(this).application_icon_badge_number = bn;
}

- (id)nextResponder {
    let delegate = msg![env; this delegate];
    let app_delegate_class = msg![env; delegate class];
    let ui_responder_class = env.objc.get_known_class("UIResponder", &mut env.mem);
    if env.objc.class_is_subclass_of(app_delegate_class, ui_responder_class) {
        delegate
    } else {
        nil
    }
}

@end

};

pub(super) fn UIApplicationMain(
    env: &mut Environment,
    _argc: i32,
    _argv: MutPtr<MutPtr<u8>>,
    principal_class_name: id,
    delegate_class_name: id,
) {
    let ui_application = {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let principal_class = if principal_class_name != nil {
            let name = ns_string::to_rust_string(env, principal_class_name);
            env.objc.get_known_class(&name, &mut env.mem)
        } else {
            env.objc.get_known_class("UIApplication", &mut env.mem)
        };
        let ui_application: id = msg![env; principal_class new];

        let device_family = env.options.device_family;
        if let Some(main_nib_filename) = env.bundle.main_nib_filename(device_family).map(str::to_owned) {
            let ns_main_nib_filename = from_rust_string(env, main_nib_filename);
            let type_: id = get_static_str(env, "nib");
            let bundle: id = msg_class![env; NSBundle mainBundle];
            let res: id = msg![env; bundle pathForResource:ns_main_nib_filename ofType:type_];
            if res != nil {
                let nib: id = msg_class![env; UINib nibWithNibName:ns_main_nib_filename bundle:nil];
                release(env, ns_main_nib_filename);
                let _: id = msg![env; nib instantiateWithOwner:ui_application options:nil];
            }
        }

        if env.bundle.status_bar_hidden() {
            let _: () = msg![env; ui_application setStatusBarHidden:true];
        }

        let delegate: id = msg![env; ui_application delegate];
        if delegate != nil {
            env.objc.borrow_mut::<UIApplicationHostObject>(ui_application).delegate_is_retained = true;
            retain(env, delegate);
        } else {
            if msg![env; delegate_class_name isEqual:principal_class_name] {
                let _: () = msg![env; ui_application setDelegate:ui_application];
            } else {
                let name = ns_string::to_rust_string(env, delegate_class_name);
                let class = env.objc.get_known_class(&name, &mut env.mem);
                let delegate: id = msg![env; class new];
                let _: () = msg![env; ui_application setDelegate:delegate];
            }
        };

        let _: () = msg![env; pool drain];
        ui_application
    };

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        
        // Retain your custom Toy Story Mania legacy loop override logic
        let has_options_method = if env.bundle.bundle_identifier().starts_with("com.disney.toystory") {
            log!("Applying Toy Story Mania Hack: Hiding didFinishLaunchingWithOptions to force legacy startup path.");
            false 
        } else {
            env.objc.object_has_method_named(&env.mem, delegate, "application:didFinishLaunchingWithOptions:")
        };

        if has_options_method {
            let empty_dict: id = msg_class![env; NSDictionary dictionary];
            let _: () = msg![env; delegate application:ui_application didFinishLaunchingWithOptions:empty_dict];
        } else if env.objc.object_has_method_named(&env.mem, delegate, "applicationDidFinishLaunching:") {
            let _: () = msg![env; delegate applicationDidFinishLaunching:ui_application];
        }
                
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidFinishLaunchingNotification);
        let _: () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    }

    let views = env.framework_state.uikit.ui_view.views.clone();
    for view in views {
        let _: () = msg![env; view layoutSubviews];
    }

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationDidBecomeActive:") {
            let _: () = msg![env; delegate applicationDidBecomeActive:ui_application];
        }
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidBecomeActiveNotification);
        let _: () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    }

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let current_device: id = msg_class![env; UIDevice currentDevice];
        let is_generating: bool = msg![env; current_device isGeneratingDeviceOrientationNotifications];
        if is_generating {
            let _: () = msg![env; current_device _postOrientationChangeNotification];
        }
        let _: () = msg![env; pool drain];
    }

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let _: () = msg![env; run_loop run];
}

pub(super) fn exit(env: &mut Environment) {
    let ui_application: id = msg_class![env; UIApplication sharedApplication];
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        if !env.is_app_picker {
            let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
            let _: bool = msg![env; user_defaults synchronize];
        }
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationWillResignActive:") {
            let _: () = msg![env; delegate applicationWillResignActive:ui_application];
        }
        let notif_name = get_static_str(env, UIApplicationWillResignActiveNotification);
        let _: () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    };
    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationWillTerminate:") {
            let _: () = msg![env; delegate applicationWillTerminate:ui_application];
        }
        let notif_name = get_static_str(env, UIApplicationWillTerminateNotification);
        let _: () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    };
    std::process::exit(0);
    }

const UIApplicationDidFinishLaunchingNotification: &str = "UIApplicationDidFinishLaunchingNotification";
const UIApplicationDidBecomeActiveNotification: &str = "UIApplicationDidBecomeActiveNotification";
const UIApplicationDidEnterBackgroundNotification: &str = "UIApplicationDidEnterBackgroundNotification";
const UIApplicationWillEnterForegroundNotification: &str = "UIApplicationWillEnterForegroundNotification";
const UIApplicationWillResignActiveNotification: &str = "UIApplicationWillResignActiveNotification";
const UIApplicationWillTerminateNotification: &str = "UIApplicationWillTerminateNotification";
const UIApplicationLaunchOptionsRemoteNotificationKey: &str = "UIApplicationLaunchOptionsRemoteNotificationKey";
const UIApplicationDidReceiveMemoryWarningNotification: &str = "UIApplicationDidReceiveMemoryWarningNotification";
const UIApplicationProtectedDataDidBecomeAvailable: &str = "UIApplicationProtectedDataDidBecomeAvailable";
const UIApplicationProtectedDataWillBecomeUnavailable: &str = "UIApplicationProtectedDataWillBecomeUnavailable";
const UIApplicationSignificantTimeChangeNotification: &str = "UIApplicationSignificantTimeChangeNotification";
const UIApplicationDidChangeStatusBarFrameNotification: &str = "UIApplicationDidChangeStatusBarFrameNotification";
const UIApplicationWillChangeStatusBarFrameNotification: &str = "UIApplicationWillChangeStatusBarFrameNotification";
const UIApplicationDidChangeStatusBarOrientationNotification: &str = "UIApplicationDidChangeStatusBarOrientationNotification";
const UIApplicationWillChangeStatusBarOrientationNotification: &str = "UIApplicationWillChangeStatusBarOrientationNotification";
const UIApplicationStatusBarFrameUserInfoKey: &str = "UIApplicationStatusBarFrameUserInfoKey";
const UIApplicationStatusBarOrientationUserInfoKey: &str = "UIApplicationStatusBarOrientationUserInfoKey";
const UIApplicationBackgroundFetchIntervalMinimum: &str = "UIApplicationBackgroundFetchIntervalMinimum";
const UIApplicationBackgroundFetchIntervalNever: &str = "UIApplicationBackgroundFetchIntervalNever";
const UIApplicationLaunchOptionsURLKey: &str = "UIApplicationLaunchOptionsURLKey";
const UIApplicationLaunchOptionsSourceApplicationKey: &str = "UIApplicationLaunchOptionsSourceApplicationKey";
const UIApplicationLaunchOptionsAnnotationKey: &str = "UIApplicationLaunchOptionsAnnotationKey";
const UIApplicationLaunchOptionsLocalNotificationKey: &str = "UIApplicationLaunchOptionsLocalNotificationKey";
const UIApplicationLaunchOptionsLocationKey: &str = "UIApplicationLaunchOptionsLocationKey";
const UIApplicationLaunchOptionsNewsstandDownloadsKey: &str = "UIApplicationLaunchOptionsNewsstandDownloadsKey";
const UIApplicationLaunchOptionsBluetoothCentralsKey: &str = "UIApplicationLaunchOptionsBluetoothCentralsKey";
const UIApplicationLaunchOptionsBluetoothPeripheralsKey: &str = "UIApplicationLaunchOptionsBluetoothPeripheralsKey";
const UIApplicationLaunchOptionsShortcutItemKey: &str = "UIApplicationLaunchOptionsShortcutItemKey";
const UIApplicationOpenSettingsURLString: &str = "app-settings:";
const UIApplicationOpenURLOptionsSourceApplicationKey: &str = "UIApplicationOpenURLOptionsSourceApplicationKey";
const UIApplicationOpenURLOptionsAnnotationKey: &str = "UIApplicationOpenURLOptionsAnnotationKey";
const UIApplicationOpenURLOptionsOpenInPlaceKey: &str = "UIApplicationOpenURLOptionsOpenInPlaceKey";
const UIApplicationOpenURLOptionUniversalLinksOnly: &str = "UIApplicationOpenURLOptionUniversalLinksOnly";

pub const CONSTANTS: ConstantExports = &[
    ("_UIApplicationDidFinishLaunchingNotification", HostConstant::NSString(UIApplicationDidFinishLaunchingNotification)),
    ("_UIApplicationDidBecomeActiveNotification", HostConstant::NSString(UIApplicationDidBecomeActiveNotification)),
    ("_UIApplicationDidEnterBackgroundNotification", HostConstant::NSString(UIApplicationDidEnterBackgroundNotification)),
    ("_UIApplicationWillEnterForegroundNotification", HostConstant::NSString(UIApplicationWillEnterForegroundNotification)),
    ("_UIApplicationWillResignActiveNotification", HostConstant::NSString(UIApplicationWillResignActiveNotification)),
    ("_UIApplicationWillTerminateNotification", HostConstant::NSString(UIApplicationWillTerminateNotification)),
    ("_UIApplicationDidReceiveMemoryWarningNotification", HostConstant::NSString(UIApplicationDidReceiveMemoryWarningNotification)),
    ("_UIApplicationLaunchOptionsRemoteNotificationKey", HostConstant::NSString(UIApplicationLaunchOptionsRemoteNotificationKey)),
    ("_UIApplicationProtectedDataDidBecomeAvailable", HostConstant::NSString(UIApplicationProtectedDataDidBecomeAvailable)),
    ("_UIApplicationProtectedDataWillBecomeUnavailable", HostConstant::NSString(UIApplicationProtectedDataWillBecomeUnavailable)),
    ("_UIApplicationSignificantTimeChangeNotification", HostConstant::NSString(UIApplicationSignificantTimeChangeNotification)),
    ("_UIApplicationDidChangeStatusBarFrameNotification", HostConstant::NSString(UIApplicationDidChangeStatusBarFrameNotification)),
    ("_UIApplicationWillChangeStatusBarFrameNotification", HostConstant::NSString(UIApplicationWillChangeStatusBarFrameNotification)),
    ("_UIApplicationDidChangeStatusBarOrientationNotification", HostConstant::NSString(UIApplicationDidChangeStatusBarOrientationNotification)),
    ("_UIApplicationWillChangeStatusBarOrientationNotification", HostConstant::NSString(UIApplicationWillChangeStatusBarOrientationNotification)),
    ("_UIApplicationStatusBarFrameUserInfoKey", HostConstant::NSString(UIApplicationStatusBarFrameUserInfoKey)),
    ("_UIApplicationStatusBarOrientationUserInfoKey", HostConstant::NSString(UIApplicationStatusBarOrientationUserInfoKey)),
    ("_UIApplicationBackgroundFetchIntervalMinimum", HostConstant::NSString(UIApplicationBackgroundFetchIntervalMinimum)),
    ("_UIApplicationBackgroundFetchIntervalNever", HostConstant::NSString(UIApplicationBackgroundFetchIntervalNever)),
    ("_UIApplicationLaunchOptionsURLKey", HostConstant::NSString(UIApplicationLaunchOptionsURLKey)),
    ("_UIApplicationLaunchOptionsSourceApplicationKey", HostConstant::NSString(UIApplicationLaunchOptionsSourceApplicationKey)),
    ("_UIApplicationLaunchOptionsAnnotationKey", HostConstant::NSString(UIApplicationLaunchOptionsAnnotationKey)),
    ("_UIApplicationLaunchOptionsLocalNotificationKey", HostConstant::NSString(UIApplicationLaunchOptionsLocalNotificationKey)),
    ("_UIApplicationLaunchOptionsLocationKey", HostConstant::NSString(UIApplicationLaunchOptionsLocationKey)),
    ("_UIApplicationLaunchOptionsNewsstandDownloadsKey", HostConstant::NSString(UIApplicationLaunchOptionsNewsstandDownloadsKey)),
    ("_UIApplicationLaunchOptionsBluetoothCentralsKey", HostConstant::NSString(UIApplicationLaunchOptionsBluetoothCentralsKey)),
    ("_UIApplicationLaunchOptionsBluetoothPeripheralsKey", HostConstant::NSString(UIApplicationLaunchOptionsBluetoothPeripheralsKey)),
    ("_UIApplicationLaunchOptionsShortcutItemKey", HostConstant::NSString(UIApplicationLaunchOptionsShortcutItemKey)),
    ("_UIApplicationOpenSettingsURLString", HostConstant::NSString(UIApplicationOpenSettingsURLString)),
    ("_UIApplicationOpenURLOptionsSourceApplicationKey", HostConstant::NSString(UIApplicationOpenURLOptionsSourceApplicationKey)),
    ("_UIApplicationOpenURLOptionsAnnotationKey", HostConstant::NSString(UIApplicationOpenURLOptionsAnnotationKey)),
    ("_UIApplicationOpenURLOptionsOpenInPlaceKey", HostConstant::NSString(UIApplicationOpenURLOptionsOpenInPlaceKey)),
    ("_UIApplicationOpenURLOptionUniversalLinksOnly", HostConstant::NSString(UIApplicationOpenURLOptionUniversalLinksOnly)),
];

pub const FUNCTIONS: FunctionExports = &[export_c_func!(UIApplicationMain(_, _, _, _))];
