/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::{ns_string, NSRange, NSUInteger};
use crate::mem::MutPtr;
use crate::msg;
use crate::objc::{
    autorelease, id, nil, objc_classes, release, ClassExports, HostObject, NSZonePtr,
};
use regex::Regex;

struct NSRegularExpressionHostObject {
    regex: Option<Regex>,
}
impl HostObject for NSRegularExpressionHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation NSRegularExpression: NSObject

    + (id)allocWithZone:(NSZonePtr)_zone {
        let host_object = Box::new(NSRegularExpressionHostObject { regex: None });
        env.objc.alloc_object(this, host_object, &mut env.mem)
    }

    + (id)regularExpressionWithPattern:(id)pattern
                               options:(u32)options
                                 error:(MutPtr<id>)error {
        let new: id = msg![env; this alloc];
        let new: id = msg![env; new initWithPattern:pattern
                                            options:options
                                              error:error];
        if new == nil {
            return nil;
        }
        autorelease(env, new)
    }

    - (id)initWithPattern:(id)pattern
                  options:(u32)_options
                    error:(MutPtr<id>)_error {
        if pattern == nil {
            release(env, this);
            return nil;
        }
        let pattern_str = ns_string::to_rust_string(env, pattern).into_owned();
        match Regex::new(&pattern_str) {
            Ok(re) => {
                env.objc
                    .borrow_mut::<NSRegularExpressionHostObject>(this)
                    .regex = Some(re);
                this
            }
            Err(e) => {
                log!("NSRegularExpression: failed to compile pattern '{}': {}", pattern_str, e);
                release(env, this);
                nil
            }
        }
    }

    - (NSUInteger)numberOfMatchesInString:(id)string
                                  options:(u32)_options
                                    range:(NSRange)range {
        if string == nil { return 0; }
        let full_text = ns_string::to_rust_string(env, string);
        let Some((start_byte, end_byte)) = utf16_range_to_utf8_byte_range(&full_text, range) else { return 0; };

        let target_text = &full_text[start_byte..end_byte];
        let host_obj = env.objc.borrow::<NSRegularExpressionHostObject>(this);
        if let Some(re) = &host_obj.regex {
            re.find_iter(target_text).count() as NSUInteger
        } else {
            0
        }
    }

    - (id)firstMatchInString:(id)string options:(u32)_options range:(NSRange)range {
        if string == nil { return nil; }
        let full_text = ns_string::to_rust_string(env, string);
        let Some((start_byte, end_byte)) = utf16_range_to_utf8_byte_range(&full_text, range) else { return nil; };

        let target_text = &full_text[start_byte..end_byte];
        let host_obj = env.objc.borrow::<NSRegularExpressionHostObject>(this);
        
        if let Some(re) = &host_obj.regex {
            if let Some(_m) = re.find(target_text) {
                // Return a dummy result object so the game thinks it found something
                let cls = env.objc.link_class("NSTextCheckingResult", false, &mut env.mem);
                return msg![env; cls alloc];
            }
        }
        nil
    }

    @end

    @implementation NSTextCheckingResult: NSObject
    // Stub for the result object to prevent crashes when the game inspects matches
    - (NSRange)range {
        NSRange { location: 0, length: 0 }
    }

    // This fix handles the game's request for specific capture groups
    - (NSRange)rangeAtIndex:(NSUInteger)_index {
        NSRange { location: 0, length: 0 }
    }
    @end
};

fn utf16_range_to_utf8_byte_range(s: &str, range: NSRange) -> Option<(usize, usize)> {
    let start_units = range.location as usize;
    let end_units = (range.location as usize).checked_add(range.length as usize)?;

    let mut units_seen: usize = 0;
    let mut start_byte: Option<usize> = None;
    let mut end_byte: Option<usize> = None;

    for (byte_offset, c) in s.char_indices() {
        if units_seen == start_units && start_byte.is_none() {
            start_byte = Some(byte_offset);
        }
        if units_seen == end_units && end_byte.is_none() {
            end_byte = Some(byte_offset);
        }
        units_seen += c.len_utf16();
    }
    if units_seen == start_units && start_byte.is_none() {
        start_byte = Some(s.len());
    }
    if units_seen == end_units && end_byte.is_none() {
        end_byte = Some(s.len());
    }
    let start_byte = start_byte?;
    let end_byte = end_byte?;
    if start_byte > end_byte {
        return None;
    }
    Some((start_byte, end_byte))
}
