/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Utilities for working with bundles. So far we are only interested in the
//! application bundle.
//!
//! Relevant Apple documentation:
//! * [Bundle Programming Guide](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/Introduction/Introduction.html)
//!   * [Anatomy of an iOS Application Bundle](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html)
//! * [Bundle Resources](https://developer.apple.com/documentation/bundleresources?language=objc)

use crate::fs::{BundleData, Fs, GuestPath, GuestPathBuf};
use crate::image::Image;
use crate::window::DeviceFamily;
use plist::dictionary::Dictionary;
use plist::Value;
use std::io::Cursor;

#[derive(Debug)]
pub struct Bundle {
    path: GuestPathBuf,
    plist: Dictionary,
}

impl Bundle {
    /// See [Fs::new] for meaning of `read_only_mode`.
    pub fn new_bundle_and_fs_from_host_path(
        mut bundle_data: BundleData,
        read_only_mode: bool,
    ) -> Result<(Bundle, Fs), String> {
        let plist_bytes = bundle_data.read_plist()?;

        let plist = Value::from_reader(Cursor::new(plist_bytes))
            .map_err(|_| "Could not deserialize plist data".to_string())?;

        let plist = plist
            .into_dictionary()
            .ok_or_else(|| "plist root value is not a dictionary".to_string())?;

        let bundle_name = format!(
            "{}.app",
            if let Some(canonical) = plist.get("CFBundleName") {
                canonical.as_string().unwrap()
            } else {
                bundle_data.bundle_name()
            }
        );
        let bundle_id = plist["CFBundleIdentifier"].as_string().unwrap();

        let (fs, guest_path) = Fs::new(bundle_data, bundle_name, bundle_id, read_only_mode);

        let bundle = Bundle {
            path: guest_path,
            plist,
        };

        Ok((bundle, fs))
    }

    /// Create a fake bundle (see [crate::Environment::new_without_app]).
    pub fn new_fake_bundle() -> Bundle {
        Bundle {
            path: GuestPathBuf::from(String::new()),
            plist: Dictionary::new(),
        }
    }

    pub fn bundle_path(&self) -> &GuestPath {
        &self.path
    }

    pub fn bundle_identifier(&self) -> &str {
        self.plist["CFBundleIdentifier"].as_string().unwrap()
    }

    pub fn bundle_version(&self) -> &str {
        self.plist["CFBundleVersion"].as_string().unwrap()
    }

    pub fn bundle_localizations(&self) -> &[Value] {
        static EMPTY_VAL: Vec<Value> = Vec::new();
        self.plist
            .get("CFBundleLocalizations")
            .and_then(|v| v.as_array())
            .unwrap_or(&EMPTY_VAL)
    }

    /// Canonical name for the bundle according to Info.plist
    pub fn canonical_bundle_name(&self) -> Option<&str> {
        self.plist
            .get("CFBundleName")
            .map(|name| name.as_string().unwrap())
    }

    /// Name for the bundle, either the canonical name or, if there isn't one,
    /// the name this bundle has in the filesystem.
    pub fn bundle_name(&self) -> &str {
        self.path.file_name().unwrap().strip_suffix(".app").unwrap()
    }

    pub fn display_name(&self) -> &str {
        if let Some(display_name) = self.plist.get("CFBundleDisplayName") {
            display_name.as_string().unwrap()
        } else {
            ""
        }
    }

    pub fn minimum_os_version(&self) -> Option<&str> {
        self.plist
            .get("MinimumOSVersion")
            .map(|v| v.as_string().unwrap())
    }

    pub fn required_device_capabilities(&self) -> Vec<&str> {
        self.plist
            .get("UIRequiredDeviceCapabilities")
            .map(|v| {
                if let Some(dict) = v.as_dictionary() {
                    dict.keys().map(|o| o.as_str()).collect()
                } else if let Some(arr) = v.as_array() {
                    arr.iter()
                        .filter_map(|o| o.as_string())
                        .collect()
                } else {
                    Vec::new()
                }
            })
            .unwrap_or_default()
    }
    
    pub fn executable_path(&self) -> GuestPathBuf {
        // FIXME: Is this key optional? All iPhone apps seem to have it.
        self.path
            .join(self.plist["CFBundleExecutable"].as_string().unwrap())
    }

    pub fn launch_image_path(&self, fs: &Fs, device_family: DeviceFamily) -> GuestPathBuf {
        // Check if there's a custom base name in plist
        let base_name = self
            .plist
            .get("UILaunchImageFile")
            .map(|v| v.as_string().unwrap())
            .unwrap_or("Default");

        // Try device-specific variants first, then fallback to base name
        let candidates = match device_family {
            DeviceFamily::iPhone5 => {
                vec![
                    format!("{}-568h@2x.png", base_name), // iPhone 5 (4-inch)
                    format!("{}@2x.png", base_name),      // iPhone Retina
                    format!("{}.png", base_name),         // iPhone non-Retina
                ]
            }
            DeviceFamily::iPad => {
                vec![
                    format!("{}@2x~ipad.png", base_name), // iPad Retina
                    format!("{}~ipad.png", base_name),    // iPad non-Retina
                    format!("{}.png", base_name),         // Fallback
                ]
            }
            DeviceFamily::iPhone => {
                vec![
                    format!("{}@2x.png", base_name), // iPhone Retina
                    format!("{}.png", base_name),    // iPhone non-Retina
                ]
            }
        };

        // Find the first existing file
        for candidate in &candidates {
            let path = self.path.join(candidate);
            if fs.read(&path).is_ok() {
                log!("Using launch image: {}", candidate);
                return path;
            }
        }

        // Final fallback
        log!("Warning: No launch image found, using Default.png");
        self.path.join("Default.png")
    }

    pub fn status_bar_hidden(&self) -> bool {
        self.plist
            .get("UIStatusBarHidden")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false)
    }

    fn icon_path(&self) -> GuestPathBuf {
        if let Some(filename) = self.plist.get("CFBundleIconFile") {
            if filename
                .as_string()
                .unwrap()
                .to_lowercase()
                .ends_with(".png")
            {
                self.path.join(filename.as_string().unwrap())
            } else {
                let filename_with_extension = format!("{}.png", filename.as_string().unwrap());
                self.path.join(filename_with_extension)
            }
        } else {
            self.path.join("Icon.png")
        }
    }

    /// Load icon and round off its corners (and add sheen if needed) for
    /// display.
    pub fn load_icon(&self, fs: &Fs) -> Result<Image, String> {
        let bytes = fs
            .read(self.icon_path())
            .map_err(|_| "Could not read icon file".to_string())?;
        let mut image =
            Image::from_bytes(&bytes).map_err(|e| format!("Could not parse icon image: {e}"))?;
        // UIPrerenderedIcon is used to avoid iOS applying a sheen effect,
        // should be boolean, but some apps use a string, so we check both.
        // See https://developer.apple.com/library/archive/qa/qa1614/_index.html
        // Default if it does not exist is NO/false.
        let add_sheen = !self
            .plist
            .get("UIPrerenderedIcon")
            .and_then(|v| v.as_boolean().or(v.as_string().map(|s| s == "YES")))
            .unwrap_or(false);
        // iPhone OS icons are 57px by 57px and the OS always applies a
        // 10px radius rounded corner (see e.g. documentation of
        // UIPrerenderedIcon). If the icon is larger for some reason,
        // let's scale to match.
        let corner_radius = (10.0 / 57.0) * (image.dimensions().0 as f32);
        image.round_corners(corner_radius, /* four_corners: */ true, add_sheen);
        Ok(image)
    }

    pub fn main_nib_filename(&self, device_family: Option<DeviceFamily>) -> Option<&str> {
        // TODO: extend this logic for all device-specific keys
        if let Some(device_family) = device_family {
            if device_family == DeviceFamily::iPad && self.plist.get("NSMainNibFile~ipad").is_some()
            {
                return self
                    .plist
                    .get("NSMainNibFile~ipad")
                    .map(|v| v.as_string().unwrap());
            }
        }
        self.plist
            .get("NSMainNibFile")
            .map(|v| v.as_string().unwrap())
    }

    pub fn supported_interface_orientations(&self) -> Vec<&str> {
        // UIInterfaceOrientation (iPhone OS 2.0) is a single string
        // (or a comma separated list of strings).
        // UISupportedInterfaceOrientations (iOS 3.2) is an array of strings and
        // takes precedence.
        self.plist
            .get("UISupportedInterfaceOrientations")
            .map(|v| {
                if let Some(arr) = v.as_array() {
                    arr.iter()
                        .map(|o| o.as_string().unwrap_or("UIInterfaceOrientationPortrait"))
                        .collect()
                } else if let Some(str) = v.as_string() {
                    log!("Warning: UISupportedInterfaceOrientations is a string, not an array. Parsing anyway.");
                    str.split(',').collect()
                } else {
                    log!("Warning: UISupportedInterfaceOrientations is a weird type. Falling back.");
                    vec!["UIInterfaceOrientationPortrait"]
                }
            })
            .unwrap_or_else(|| {
                if let Some(v) = self
                    .plist
                    .get("UIInterfaceOrientation") {
                    let str = v.as_string().unwrap_or("UIInterfaceOrientationPortrait");
                    if str.contains(',') {
                        log!("UIInterfaceOrientation is a comma separated list of strings ({}), splitting!", str);
                    }
                    str.split(',').collect()
                } else {
                    vec!["UIInterfaceOrientationPortrait"]
                }
            })
    }

    pub fn device_family_array(&self) -> Vec<DeviceFamily> {
        self.plist
            .get("UIDeviceFamily")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|o| o.as_unsigned_integer())
                    .filter_map(|num| DeviceFamily::try_from(num).ok())
                    .collect()
            })
            .unwrap_or_else(|| vec![DeviceFamily::iPhone])
    }
} // <--- This was the missing closing brace that fixes the build!
