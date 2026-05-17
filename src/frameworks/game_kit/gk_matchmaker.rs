/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `GKMatchmaker`.

use crate::objc::{id, objc_classes, ClassExports, HostObject, NSZonePtr};

struct GKMatchmakerHostObject;
impl HostObject for GKMatchmakerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation GKMatchmaker: NSObject

    // =========================================================================
    // MARK: - Singleton
    // =========================================================================

    + (id)sharedMatchmaker {
        log!("GKMatchmaker sharedMatchmaker [STUB] -> Returning mock singleton to bypass freeze");
        let host_object = Box::new(GKMatchmakerHostObject);
        env.objc.alloc_object(this, host_object, &mut env.mem)
    }

    // =========================================================================
    // MARK: - Stubs for common matchmaking chain calls
    // =========================================================================

    - (())startBrowsingForNearbyPlayersWithHandler:(id)_handler {
        log_dbg!("GKMatchmaker startBrowsingForNearbyPlayersWithHandler: [STUB]");
    }

    - (())cancel {
        log_dbg!("GKMatchmaker cancel [STUB]");
    }

    @end
};
