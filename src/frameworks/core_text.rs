/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CoreText.framework/CoreText`
//!
//! CoreText is the C-level text rendering API exposed by macOS and iOS
//! since iOS 3.2. We don't currently implement layout/glyph metrics
//! (apps that need full text shaping fall back to UIKit/UIFont), but
//! many apps reference the framework's exported string constants —
//! attribute keys used when constructing font descriptor dictionaries
//! (`CTFontDescriptorCreateWithAttributes`) — purely for `isEqual:`
//! comparisons against keys returned by the system.
//!
//! Per Apple's `CTFontDescriptor.h`/`CTFont.h` headers these are
//! `CFStringRef` constants with the canonical string value identical
//! to the symbol's spelling (e.g. `kCTFontNameAttribute` == the
//! `CFStringRef` for the C string "NSCTFontNameAttribute" — actually
//! "NSFontNameAttribute" in modern SDKs). For touchHLE's purposes the
//! exact textual content only matters for identity comparisons; we
//! mirror the spelling Apple's public headers document.
//!
//! References:
//! - <https://developer.apple.com/documentation/coretext/font_descriptor_attribute_keys>
//! - `CTFontDescriptor.h`, `CTFont.h`, `CTFontTraits.h` (Apple SDK).

use crate::dyld::{ConstantExports, HostConstant, HostDylib};

pub const CONSTANTS: ConstantExports = &[
    // CTFontDescriptor.h
    (
        "_kCTFontNameAttribute",
        HostConstant::NSString("NSFontNameAttribute"),
    ),
    (
        "_kCTFontFamilyNameAttribute",
        HostConstant::NSString("NSFontFamilyAttribute"),
    ),
    (
        "_kCTFontStyleNameAttribute",
        HostConstant::NSString("NSFontFaceAttribute"),
    ),
    (
        "_kCTFontTraitsAttribute",
        HostConstant::NSString("NSCTFontTraitsAttribute"),
    ),
    (
        "_kCTFontURLAttribute",
        HostConstant::NSString("NSCTFontFileURLAttribute"),
    ),
    (
        "_kCTFontDisplayNameAttribute",
        HostConstant::NSString("NSFontVisibleNameAttribute"),
    ),
    (
        "_kCTFontSizeAttribute",
        HostConstant::NSString("NSFontSizeAttribute"),
    ),
    (
        "_kCTFontMatrixAttribute",
        HostConstant::NSString("NSCTFontMatrixAttribute"),
    ),
    (
        "_kCTFontCascadeListAttribute",
        HostConstant::NSString("NSCTFontCascadeListAttribute"),
    ),
    (
        "_kCTFontCharacterSetAttribute",
        HostConstant::NSString("NSCTFontCharacterSetAttribute"),
    ),
    (
        "_kCTFontLanguagesAttribute",
        HostConstant::NSString("NSCTFontLanguagesAttribute"),
    ),
    (
        "_kCTFontBaselineAdjustAttribute",
        HostConstant::NSString("NSCTFontBaselineAdjustAttribute"),
    ),
    (
        "_kCTFontMacintoshEncodingsAttribute",
        HostConstant::NSString("NSCTFontMacintoshEncodingsAttribute"),
    ),
    (
        "_kCTFontFeaturesAttribute",
        HostConstant::NSString("NSCTFontFeaturesAttribute"),
    ),
    (
        "_kCTFontFeatureSettingsAttribute",
        HostConstant::NSString("NSCTFontFeatureSettingsAttribute"),
    ),
    (
        "_kCTFontFixedAdvanceAttribute",
        HostConstant::NSString("NSCTFontFixedAdvanceAttribute"),
    ),
    (
        "_kCTFontOrientationAttribute",
        HostConstant::NSString("NSCTFontOrientationAttribute"),
    ),
    (
        "_kCTFontFormatAttribute",
        HostConstant::NSString("NSCTFontFormatAttribute"),
    ),
    (
        "_kCTFontRegistrationScopeAttribute",
        HostConstant::NSString("NSCTFontRegistrationScopeAttribute"),
    ),
    (
        "_kCTFontPriorityAttribute",
        HostConstant::NSString("NSCTFontPriorityAttribute"),
    ),
    (
        "_kCTFontEnabledAttribute",
        HostConstant::NSString("NSCTFontEnabledAttribute"),
    ),
    (
        "_kCTFontDownloadableAttribute",
        HostConstant::NSString("NSCTFontDownloadableAttribute"),
    ),
    (
        "_kCTFontDownloadedAttribute",
        HostConstant::NSString("NSCTFontDownloadedAttribute"),
    ),
    // CTFontTraits.h
    (
        "_kCTFontSymbolicTrait",
        HostConstant::NSString("NSCTFontSymbolicTrait"),
    ),
    (
        "_kCTFontWeightTrait",
        HostConstant::NSString("NSCTFontWeightTrait"),
    ),
    (
        "_kCTFontWidthTrait",
        HostConstant::NSString("NSCTFontWidthTrait"),
    ),
    (
        "_kCTFontSlantTrait",
        HostConstant::NSString("NSCTFontSlantTrait"),
    ),
];

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};
