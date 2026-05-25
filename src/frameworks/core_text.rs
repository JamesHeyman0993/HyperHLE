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
//! either as attribute keys when constructing font descriptor
//! dictionaries (`CTFontDescriptorCreateWithAttributes`) or as
//! attribute keys when building `CFAttributedStringRef` /
//! `NSAttributedString` instances for `CTFramesetterCreateWithAttributedString`
//! and friends — purely for `isEqual:` comparisons against keys
//! returned by the system.
//!
//! Per Apple's `CTFontDescriptor.h`, `CTFont.h`, `CTFontTraits.h` and
//! `CTStringAttributes.h` headers these are `CFStringRef` constants
//! with a canonical string value. For attributed-string attribute
//! keys most of them have the same value as their `NSAttributedString`
//! counterpart (e.g. `kCTFontAttributeName` == the `CFStringRef` for
//! the C string "NSFont", which is the same as the AppKit / UIKit
//! `NSFontAttributeName`), which is what makes CoreText / Foundation
//! attributed strings toll-free bridgeable. For touchHLE's purposes
//! the exact textual content only matters for identity comparisons;
//! we mirror the spelling Apple's public headers document.
//!
//! References:
//! - <https://developer.apple.com/documentation/coretext/font_descriptor_attribute_keys>
//! - <https://developer.apple.com/documentation/coretext/core_text_string_attributes>
//! - `CTFontDescriptor.h`, `CTFont.h`, `CTFontTraits.h`,
//!   `CTStringAttributes.h` (Apple SDK).

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
    // CTStringAttributes.h — attribute keys for `CFAttributedStringRef`
    // (and, toll-free bridged, `NSAttributedString`) used by
    // `CTFramesetterCreateWithAttributedString` and friends.
    // Canonical string values come from Apple's public
    // `CTStringAttributes.h` header; many deliberately share their
    // value with the corresponding `NSAttributedString` attribute name
    // so the same dictionary can be used by both CoreText and
    // UIKit/AppKit.
    (
        "_kCTFontAttributeName",
        // Same value as `NSFontAttributeName`.
        HostConstant::NSString("NSFont"),
    ),
    (
        "_kCTForegroundColorAttributeName",
        HostConstant::NSString("CTForegroundColor"),
    ),
    (
        "_kCTForegroundColorFromContextAttributeName",
        HostConstant::NSString("CTForegroundColorFromContext"),
    ),
    (
        "_kCTBackgroundColorAttributeName",
        HostConstant::NSString("kCTBackgroundColorAttributeName"),
    ),
    (
        "_kCTKernAttributeName",
        // Same value as `NSKernAttributeName`.
        HostConstant::NSString("NSKern"),
    ),
    (
        "_kCTLigatureAttributeName",
        // Same value as `NSLigatureAttributeName`.
        HostConstant::NSString("NSLigature"),
    ),
    (
        "_kCTParagraphStyleAttributeName",
        // Same value as `NSParagraphStyleAttributeName`.
        HostConstant::NSString("NSParagraphStyle"),
    ),
    (
        "_kCTStrokeWidthAttributeName",
        // Same value as `NSStrokeWidthAttributeName`.
        HostConstant::NSString("NSStrokeWidth"),
    ),
    (
        "_kCTStrokeColorAttributeName",
        // Same value as `NSStrokeColorAttributeName`.
        HostConstant::NSString("NSStrokeColor"),
    ),
    (
        "_kCTUnderlineStyleAttributeName",
        HostConstant::NSString("CTUnderlineStyle"),
    ),
    (
        "_kCTUnderlineColorAttributeName",
        HostConstant::NSString("CTUnderlineColor"),
    ),
    (
        "_kCTSuperscriptAttributeName",
        // Same value as `NSSuperscriptAttributeName`.
        HostConstant::NSString("NSSuperScript"),
    ),
    (
        "_kCTVerticalFormsAttributeName",
        HostConstant::NSString("CTVerticalForms"),
    ),
    (
        "_kCTGlyphInfoAttributeName",
        HostConstant::NSString("CTGlyphInfo"),
    ),
    (
        "_kCTCharacterShapeAttributeName",
        // Same value as `NSCharacterShapeAttributeName`.
        HostConstant::NSString("NSCharacterShape"),
    ),
    (
        "_kCTLanguageAttributeName",
        HostConstant::NSString("CTLanguage"),
    ),
    (
        "_kCTRunDelegateAttributeName",
        HostConstant::NSString("CTRunDelegate"),
    ),
    (
        "_kCTBaselineClassAttributeName",
        HostConstant::NSString("CTBaselineClass"),
    ),
    (
        "_kCTBaselineInfoAttributeName",
        HostConstant::NSString("CTBaselineInfo"),
    ),
    (
        "_kCTBaselineReferenceInfoAttributeName",
        HostConstant::NSString("CTBaselineReferenceInfo"),
    ),
    (
        "_kCTBaselineOffsetAttributeName",
        // Same value as `NSBaselineOffsetAttributeName`.
        HostConstant::NSString("NSBaselineOffset"),
    ),
    (
        "_kCTWritingDirectionAttributeName",
        // Same value as `NSWritingDirectionAttributeName`.
        HostConstant::NSString("NSWritingDirection"),
    ),
    (
        "_kCTTrackingAttributeName",
        HostConstant::NSString("CTTracking"),
    ),
];

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};
