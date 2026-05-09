/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIViewController`.
//!
//! Resources:
//! - [View Controller Programming Guide for iOS (Legacy)](https://developer.apple.com/library/archive/documentation/WindowsViews/Conceptual/ViewControllerPGforiOSLegacy/BasicViewControllers/BasicViewControllers.html)

use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::ns_objc_runtime::NSStringFromClass;
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::NSUInteger;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientation, UIInterfaceOrientationPortrait,
};
use crate::frameworks::uikit::ui_view::set_view_controller;
use crate::objc::{
    autorelease, id, msg, msg_class, msg_super, nil, objc_classes, release, retain,
    todo_objc_setter, Class, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;

pub mod ui_navigation_controller;

pub type UIModalTransitionStyle = i32;
pub type UIModalPresentationStyle = i32;

#[derive(Default)]
struct UIViewControllerHostObject {
    /// The root view.
    /// `UIView*`
    view: id,
    /// Nib name to be used at the load
    /// of the root view, may be nil.
    /// `NSString*`
    nib_name: id,
    /// Bundle to be used for load
    /// of the nib by name, may be nil.
    /// `NSBundle*`
    bundle: id,
    title: id,                  // Для хранения строки заголовка
    parent_view_controller: id, // Для связи с родительским VC
    navigation_controller: id,  // Для фикса ошибки "does not respond to selector"
    /// Currently presented modal view controller (retained), or `nil`.
    /// `UIViewController*`
    presented_view_controller: id,
    /// View controller that is presenting `self` modally (non-retained
    /// back-pointer), or `nil`.
    /// `UIViewController*`
    presenting_view_controller: id,
    /// Lazily-created `UINavigationItem` returned by `-navigationItem`.
    /// Retained while it lives in this slot.
    navigation_item: id,
    // ---------------------------
    modal_transition_style: UIModalTransitionStyle,
    modal_presentation_style: UIModalPresentationStyle,
}
impl HostObject for UIViewControllerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIViewController: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIViewControllerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)navigationController {
    env.objc.borrow::<UIViewControllerHostObject>(this).navigation_controller
}

- (id)parentViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).parent_view_controller
}

- (id)initWithNibName:(id)nib_name // NSString *
               bundle:(id)bundle { // NSBundle *
    if nib_name != nil {
        retain(env, nib_name);
    }
    if bundle != nil {
        retain(env, bundle);
    }

    log_dbg!("[(UIViewController*){:?} initWithNibName:{:?} bundle:{:?}]", this, nib_name, bundle);

    env.objc.borrow_mut::<UIViewControllerHostObject>(this).nib_name = nib_name;
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).bundle = bundle;
    this
}

- (id)initWithCoder:(id)coder {
    let key_ns_string = get_static_str(env, "UIView");
    let view: id = msg![env; coder decodeObjectForKey:key_ns_string];
    () = msg![env; this setView:view];

    let nib_name_key = get_static_str(env, "UINibName");
    let mut nib_name: id = msg![env; coder decodeObjectForKey:nib_name_key];

    if nib_name == nil {
        let sb_name_key = get_static_str(env, "UIStoryboardName");
        nib_name = msg![env; coder decodeObjectForKey:sb_name_key];
    }

    if nib_name != nil {
        retain(env, nib_name);
        let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
        let old_nib_name = host_obj.nib_name;
        host_obj.nib_name = nib_name;
        release(env, old_nib_name);
    }

    let bundle_key = get_static_str(env, "UIBundleName");
    let bundle: id = msg![env; coder decodeObjectForKey:bundle_key];
    if bundle != nil {
        retain(env, bundle);
        let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
        let old_bundle = host_obj.bundle;
        host_obj.bundle = bundle;
        release(env, old_bundle);
    }

    this
}

- (())dealloc {
    let &UIViewControllerHostObject {
        view, nib_name, bundle, title, presented_view_controller, ..
    } = env.objc.borrow(this);
    if view != nil { release(env, view); }
    if nib_name != nil { release(env, nib_name); }
    if bundle != nil { release(env, bundle); }
    if title != nil { release(env, title); }
    if presented_view_controller != nil { release(env, presented_view_controller); }
    let navigation_item = env.objc.borrow::<UIViewControllerHostObject>(this).navigation_item;
    if navigation_item != nil { release(env, navigation_item); }

    env.objc.dealloc_object(this, &mut env.mem);
}

- (())setParentViewController:(id)parent {
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).parent_view_controller = parent;
}

- (())setNavigationController:(id)nav_controller {
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).navigation_controller = nav_controller;
}

- (())loadView {
    let nib_name: id = msg![env; this nibName];
    let mut bundle: id = msg![env; this nibBundle];
    if bundle == nil {
        bundle = msg_class![env; NSBundle mainBundle];
    }

    if nib_name != nil {
        let nib: id = msg_class![env; UINib nibWithNibName:nib_name bundle:bundle];
        if nib != nil {
            () = msg![env; nib instantiateWithOwner:this options:nil];
            if env.objc.borrow::<UIViewControllerHostObject>(this).view != nil {
                return;
            }
        }
    }

    let screen = msg_class![env; UIScreen mainScreen];
    let bounds: CGRect = msg![env; screen bounds];
    let view = msg_class![env; UIView alloc];
    let view: id = msg![env; view initWithFrame:bounds];
    () = msg![env; this setView:view];
    release(env, view);
}

- (())setView:(id)new_view {
    let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
    let old_view = std::mem::replace(&mut host_obj.view, new_view);
    if old_view != nil {
        set_view_controller(env, old_view, nil);
    }
    if new_view != nil {
        set_view_controller(env, new_view, this);
    }
    if new_view != nil {
        retain(env, new_view);
    }
    if old_view != nil {
        release(env, old_view);
    }
}

- (id)view {
    let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
    if view == nil {
        () = msg![env; this loadView];
        () = msg![env; this viewDidLoad];
        env.objc.borrow::<UIViewControllerHostObject>(this).view
    } else {
        view
    }
}

- (())setValue:(id)value forKey:(id)key {
    let key_str = to_rust_string(env, key);
    if key_str == "view" {
        () = msg![env; this setView:value];
    } else {
        () = msg_super![env; this setValue:value forKey:key];
    }
}

- (())viewDidLoad {}
- (())viewWillAppear:(bool)_animated {}
- (())viewDidAppear:(bool)_animated {}
- (())viewWillDisappear:(bool)_animated {}
- (())viewDidDisappear:(bool)_animated {}

- (())setTitle:(id)title {
    let old_title = env.objc.borrow::<UIViewControllerHostObject>(this).title;
    if old_title != nil { release(env, old_title); }
    if title != nil { retain(env, title); }
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).title = title;
}

- (())setEditing:(bool)editing {
    todo_objc_setter!(this, editing);
}
- (())setWantsFullScreenLayout:(bool)wants {
    todo_objc_setter!(this, wants);
}

- (())dismissModalViewControllerAnimated:(bool)animated {
    let presented = env.objc.borrow::<UIViewControllerHostObject>(this).presented_view_controller;
    if presented == nil {
        let presenter = env.objc.borrow::<UIViewControllerHostObject>(this).presenting_view_controller;
        if presenter != nil {
            return msg![env; presenter dismissModalViewControllerAnimated:animated];
        }
        return;
    }
    () = msg![env; presented viewWillDisappear:animated];
    let presented_view: id = msg![env; presented view];
    if presented_view != nil {
        () = msg![env; presented_view removeFromSuperview];
    }
    () = msg![env; presented viewDidDisappear:animated];
    env.objc.borrow_mut::<UIViewControllerHostObject>(presented).presenting_view_controller = nil;
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).presented_view_controller = nil;
    release(env, presented);
}

- (bool)shouldAutorotateToInterfaceOrientation:(UIInterfaceOrientation)interface_orientation {
    interface_orientation == 3 || interface_orientation == 4
}

- (id)nextResponder {
    let view = msg![env; this view];
    msg![env; view superview]
}

- (id)title {
    let stored_title = env.objc.borrow::<UIViewControllerHostObject>(this).title;
    if stored_title != nil {
        stored_title
    } else {
        let class: Class = msg![env; this class];
        NSStringFromClass(env, class)
    }
}

- (bool)isViewLoaded {
    env.objc.borrow::<UIViewControllerHostObject>(this).view != nil
}

- (id)nibName {
    let host = env.objc.borrow::<UIViewControllerHostObject>(this);
    if host.nib_name != nil {
        return host.nib_name;
    }
    let mut bundle: id = msg![env; this nibBundle];
    if bundle == nil {
        bundle = msg_class![env; NSBundle mainBundle];
    }
    let class: Class = msg![env; this class];
    let class_name: id = NSStringFromClass(env, class);
    let resolved = resolve_nib_name_from_class(env, bundle, class_name);
    release(env, class_name);
    if resolved != nil {
        retain(env, resolved);
        env.objc.borrow_mut::<UIViewControllerHostObject>(this).nib_name = resolved;
    }
    resolved
}

- (id)nibBundle {
    env.objc.borrow::<UIViewControllerHostObject>(this).bundle
}

- (())viewDidUnload {}
- (())viewWillLayoutSubviews {}
- (())viewDidLayoutSubviews {}
- (bool)isEditing { false }

- (())setEditing:(bool)editing animated:(bool)_animated {
    msg![env; this setEditing:editing]
}

- (id)editButtonItem {
    msg_class![env; UIBarButtonItem new]
}

- (())presentModalViewController:(id)modal_vc animated:(bool)animated {
    if modal_vc == nil { return; }
    retain(env, modal_vc);
    let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
    let old_modal = std::mem::replace(&mut host_obj.presented_view_controller, modal_vc);
    if old_modal != nil { release(env, old_modal); }
    env.objc.borrow_mut::<UIViewControllerHostObject>(modal_vc).presenting_view_controller = this;

    let modal_view: id = msg![env; modal_vc view];
    let presenter_view: id = msg![env; this view];
    let mut window: id = if presenter_view != nil {
        msg![env; presenter_view window]
    } else {
        nil
    };
    if window == nil {
        let app: id = msg_class![env; UIApplication sharedApplication];
        window = msg![env; app keyWindow];
    }
    if window == nil { return; }

    () = msg![env; modal_vc viewWillAppear:animated];
    () = msg![env; window addSubview:modal_view];
    () = msg![env; modal_vc viewDidAppear:animated];
}

- (id)modalViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).presented_view_controller
}

- (id)presentingViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).presenting_view_controller
}

- (id)presentedViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).presented_view_controller
}

- (())presentViewController:(id)vc animated:(bool)animated completion:(id)_completion {
    msg![env; this presentModalViewController:vc animated:animated]
}

- (())dismissViewControllerAnimated:(bool)animated completion:(id)_completion {
    msg![env; this dismissModalViewControllerAnimated:animated]
}

- (bool)wantsFullScreenLayout { false }
- (bool)hidesBottomBarWhenPushed { false }
- (())setHidesBottomBarWhenPushed:(bool)_value {}

- (id)tabBarItem {
    msg_class![env; UITabBarItem new]
}

- (())setTabBarItem:(id)_item {}
- (id)tabBarController { nil }
- (id)interfaceOrientation { nil }

- (id)navigationItem {
    let existing = env.objc.borrow::<UIViewControllerHostObject>(this).navigation_item;
    if existing != nil { return existing; }
    let title: id = msg![env; this title];
    let init_title = if title != nil {
        title
    } else {
        let class: Class = msg![env; this class];
        NSStringFromClass(env, class)
    };
    let item: id = msg_class![env; UINavigationItem alloc];
    let item: id = msg![env; item initWithTitle:init_title];
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).navigation_item = item;
    item
}

- (())didReceiveMemoryWarning {
    let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
    if view != nil {
        let superview: id = msg![env; view superview];
        if superview == nil {
            () = msg![env; this viewDidUnload];
            () = msg![env; this setView:nil];
        }
    }
}

- (bool)shouldAutorotate { true }
- (NSUInteger)supportedInterfaceOrientations { 0xFF }
- (UIInterfaceOrientation)preferredInterfaceOrientationForPresentation { 3 }
- (id)childViewControllers { msg_class![env; NSArray new] }
- (())addChildViewController:(id)_child {}
- (())removeFromParentViewController {}
- (())willMoveToParentViewController:(id)_parent {}
- (())didMoveToParentViewController:(id)_parent {}
- (())beginAppearanceTransition:(bool)_appearing animated:(bool)_animated {}
- (())endAppearanceTransition {}
- (bool)shouldAutomaticallyForwardAppearanceMethods { true }

@end

// --- STUBS FOR GAME STABILITY ---

@implementation UITabBarController: UIViewController

- (())setViewControllers:(id)_controllers {
    log!("[STUB] UITabBarController setViewControllers called");
}

- (())setViewControllers:(id)_controllers animated:(bool)_animated {
    log!("[STUB] UITabBarController setViewControllers:animated: called");
}

- (id)viewControllers {
    msg_class![env; NSArray new]
}

- (id)selectedViewController { nil }
- (())setSelectedViewController:(id)_vc {}

@end

@implementation VideoViewController: UIViewController

- (())viewDidAppear:(bool)animated {
    () = msg_super![env; this viewDidAppear:animated];
    log!("[HACK] VideoViewController auto-closing!");
    () = msg![env; this dismissModalViewControllerAnimated:false];
    let view: id = msg![env; this view];
    if view != nil { () = msg![env; view removeFromSuperview]; }
}

@end

@implementation BarcodeReaderViewController: UIViewController

- (())viewDidAppear:(bool)animated {
    () = msg_super![env; this viewDidAppear:animated];
    log!("[HACK] BarcodeReaderViewController auto-closing!");
    () = msg![env; this dismissModalViewControllerAnimated:false];
    let view: id = msg![env; this view];
    if view != nil { () = msg![env; view removeFromSuperview]; }
}

@end

}; // End of CLASSES macro

fn check_and_resolve_nib(env: &mut Environment, bundle: id, base_name: id) -> id {
    if base_name == nil { return nil; }
    let type_: id = get_static_str(env, "nib");
    let base_name_str = to_rust_string(env, base_name);
    let suffixes = ["", "~iphone", "~ipad", "-iPhone", "-iPad", "_iPhone", "_iPad"];
    for suffix in &suffixes {
        let candidate = format!("{}{}", base_name_str, suffix);
        let candidate_ns: id = from_rust_string(env, candidate);
        let path: id = msg![env; bundle pathForResource:candidate_ns ofType:type_];
        if path != nil {
            release(env, path);
            return autorelease(env, candidate_ns);
        }
        release(env, candidate_ns);
    }
    nil
}

fn resolve_nib_name_from_class(env: &mut Environment, bundle: id, class_name: id) -> id {
    if class_name == nil { return nil; }
    let res = check_and_resolve_nib(env, bundle, class_name);
    if res != nil { return res; }
    let class_str = to_rust_string(env, class_name);
    if class_str.ends_with("Controller") {
        let short_name = &class_str[..class_str.len() - "Controller".len()];
        let short_ns = from_rust_string(env, short_name.to_string());
        let res = check_and_resolve_nib(env, bundle, short_ns);
        release(env, short_ns);
        if res != nil { return res; }
    }
    nil
}
