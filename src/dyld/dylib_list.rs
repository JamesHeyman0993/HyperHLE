/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Separate module just for the dylib list, so it gets its own git history.

use crate::frameworks;
use crate::frameworks::libsqlite3;
use crate::libc;
use crate::objc;

// CoreAudio
pub const CORE_AUDIO: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CoreAudio.framework/CoreAudio",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::core_audio::FUNCTIONS],
};

// CFNetwork
pub const CF_NETWORK: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CFNetwork.framework/CFNetwork",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[frameworks::cf_network::CONSTANTS],
    function_exports: &[frameworks::cf_network::FUNCTIONS],
};

// MobileCoreServices (stub — no UTType implementation yet)
pub const MOBILE_CORE_SERVICES: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MobileCoreServices.framework/MobileCoreServices",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[frameworks::mobile_core_services::CONSTANTS],
    function_exports: &[frameworks::mobile_core_services::FUNCTIONS],
};

// CoreMedia (stub — function exports are currently registered with CoreVideo)
pub const CORE_MEDIA: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CoreMedia.framework/CoreMedia",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::core_media::FUNCTIONS],
};

// MapKit (stub — no real map rendering yet, just satisfies the dependency)
pub const MAP_KIT: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MapKit.framework/MapKit",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::map_kit::FUNCTIONS],
};

// MessageUI (stub — no real compose-sheet implementation yet)
pub const MESSAGE_UI: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MessageUI.framework/MessageUI",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::message_ui::FUNCTIONS],
};

// AddressBookUI (stub — no real contacts-picker implementation yet)
pub const ADDRESS_BOOK_UI: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/AddressBookUI.framework/AddressBookUI",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::address_book_ui::FUNCTIONS],
};

// CoreData (Updated with constants to prevent Ghost Toasters crash)
pub const CORE_DATA: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CoreData.framework/CoreData",
    aliases: &[],
    class_exports: &[frameworks::core_data::CLASSES],
    constant_exports: &[frameworks::core_data::CONSTANTS],
    function_exports: &[],
};

/// The single list of host dylibs that the linker (and Objective-C runtime)
/// searches through.
pub const DYLIB_LIST: &[&super::HostDylib] = &[
    &libc::DYLIB,
    &objc::DYLIB,
    &crate::environment::app_picker::DYLIB, // Not a real library; special internal classes.
    &frameworks::audio_toolbox::DYLIB,
    &frameworks::avfoundation::DYLIB,
    &frameworks::core_animation::DYLIB,
    &frameworks::core_foundation::DYLIB,
    &frameworks::core_graphics::DYLIB,
    &frameworks::core_location::DYLIB,
    &frameworks::core_motion::DYLIB,
    &frameworks::foundation::DYLIB,
    &frameworks::game_kit::DYLIB,
    &frameworks::media_player::DYLIB,
    &frameworks::openal::DYLIB,
    &frameworks::opengles::DYLIB,
    &frameworks::security::DYLIB,
    &frameworks::store_kit::DYLIB,
    &frameworks::system_configuration::DYLIB,
    &frameworks::uikit::DYLIB,
    &frameworks::libicucore::DYLIB,
    &frameworks::libsqlite3::DYLIB,
    &frameworks::libxml2::DYLIB,
    &frameworks::libbz2::DYLIB,
    &frameworks::common_crypto::DYLIB,
    &frameworks::core_video::DYLIB,
    &frameworks::address_book::DYLIB,
    &frameworks::game_controller::DYLIB,
    &CORE_AUDIO,
    &CF_NETWORK,
    &MOBILE_CORE_SERVICES,
    &CORE_MEDIA,
    &MAP_KIT,
    &MESSAGE_UI,
    &ADDRESS_BOOK_UI,
    &CORE_DATA,
];

// ... rest of the file (tests) remains exactly the same ...
