/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::objc::{id, nil, objc_classes, ClassExports, NSZonePtr};
// Bring UIViewController into scope so the macro can see it cleanly
use crate::frameworks::uikit::ui_view_controller::UIViewController;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SKProduct: NSObject
// TODO
@end

@implementation SKProductsRequest: NSObject
- (id)initWithProductIdentifiers:(id)_ids { // NSSet *
    // TODO
    nil
}
@end

// FIX: Stub out SKStoreProductViewController so the dynamic linker 
// stops warning us when games look up the class method.
@implementation SKStoreProductViewController: UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    // Return nil to signal that the store popup is unavailable
    nil
}

@end

};
