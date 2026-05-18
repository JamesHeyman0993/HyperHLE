/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Separate module just for the dylib list, so it gets its own git history.

use crate::frameworks;
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

// MobileCoreServices
pub const MOBILE_CORE_SERVICES: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MobileCoreServices.framework/MobileCoreServices",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[frameworks::mobile_core_services::CONSTANTS],
    function_exports: &[frameworks::mobile_core_services::FUNCTIONS],
};

// CoreMedia
// FIXED: Linked directly to your framework file configuration block so your new constants register
pub const CORE_MEDIA: super::HostDylib = frameworks::core_media::DYLIB;

// MapKit
pub const MAP_KIT: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MapKit.framework/MapKit",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::map_kit::FUNCTIONS],
};

// MessageUI
pub const MESSAGE_UI: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/MessageUI.framework/MessageUI",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::message_ui::FUNCTIONS],
};

// AddressBookUI
pub const ADDRESS_BOOK_UI: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/AddressBookUI.framework/AddressBookUI",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[frameworks::address_book_ui::FUNCTIONS],
};

// CoreData
pub const CORE_DATA: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/CoreData.framework/CoreData",
    aliases: &[],
    class_exports: &[frameworks::core_data::CLASSES],
    constant_exports: &[frameworks::core_data::CONSTANTS],
    function_exports: &[],
};

// --- ADDED EVENTKIT MOCK STUB ---
pub const EVENT_KIT: super::HostDylib = super::HostDylib {
    path: "/System/Library/Frameworks/EventKit.framework/EventKit",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[],
};

/// The single list of host dylibs that the linker searches through.
pub const DYLIB_LIST: &[&super::HostDylib] = &[
    &libc::DYLIB,
    &objc::DYLIB,
    &crate::environment::app_picker::DYLIB, 
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
    &frameworks::user_voice::DYLIB, 
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
    &frameworks::core_telephony::DYLIB,
    &EVENT_KIT, // Added our new reference here at the bottom
];
