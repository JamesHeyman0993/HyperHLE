/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stub for `Social.framework/Social`.
//!
//! On iOS 6+, Social provides `SLComposeViewController` and the
//! `SLServiceType*` string identifiers used to talk to system-level Twitter,
//! Facebook, Sina/Tencent Weibo and LinkedIn accounts.
//!
//! touchHLE only targets pre-iOS-4-era games today, but many later apps still
//! list Social as a Mach-O dependency (often pulled in transitively by an
//! analytics or sharing SDK such as Flurry or Facebook Audience Network) and
//! reference its `SLServiceType*` constants by symbol name.  Without a
//! [crate::dyld::HostDylib] entry, those constants stay NULL and any
//! guest-side `[NSString isEqualToString:SLServiceTypeFacebook]` crashes the
//! emulator with a NULL-page read.  This stub satisfies the dependency and
//! returns plain NSString constants whose values are the canonical Apple
//! identifiers.

use crate::dyld::{ConstantExports, FunctionExports, HostConstant};

pub const CONSTANTS: ConstantExports = &[
    (
        "_SLServiceTypeTwitter",
        HostConstant::NSString("com.apple.social.twitter"),
    ),
    (
        "_SLServiceTypeFacebook",
        HostConstant::NSString("com.apple.social.facebook"),
    ),
    (
        "_SLServiceTypeSinaWeibo",
        HostConstant::NSString("com.apple.social.sinaweibo"),
    ),
    (
        "_SLServiceTypeTencentWeibo",
        HostConstant::NSString("com.apple.social.tencentweibo"),
    ),
    (
        "_SLServiceTypeLinkedIn",
        HostConstant::NSString("com.apple.social.linkedin"),
    ),
];

pub const FUNCTIONS: FunctionExports = &[];
