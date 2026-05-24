/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! `NSURLConnection`.
//!
//! This is a stub implementation that does not perform real networking.
//!
//! Synchronous requests return empty NSData with a descriptive NSError.
//! Asynchronous connections immediately call `connection:didFailWithError:`
//! on the delegate (if it implements that method) so the app can handle
//! the failure gracefully instead of hanging or crashing.

use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};

// NSError domain / code used when reporting "no network in emulator".
const NS_URL_ERROR_DOMAIN: &str = "NSURLErrorDomain";
const NS_URL_ERROR_NOT_CONNECTED_TO_INTERNET: i32 = -1009;

// ---------------------------------------------------------------------------
// Host object — stores the delegate so we can call it back.
// ---------------------------------------------------------------------------

struct NSURLConnectionHostObject {
    /// `id<NSURLConnectionDelegate>` — retained while the connection is
    /// alive, released on dealloc / cancel.
    delegate: id,
    /// Whether the connection has already been cancelled / finished.
    cancelled: bool,
}
impl HostObject for NSURLConnectionHostObject {}

// ---------------------------------------------------------------------------
// Helper — build an NSError for "not connected to internet".
// ---------------------------------------------------------------------------
fn make_network_error(env: &mut crate::Environment) -> id {
    use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str};

    let domain = from_rust_string(env, NS_URL_ERROR_DOMAIN.to_string());
    autorelease(env, domain);

    let desc_key = get_static_str(env, "NSLocalizedDescription");
    let desc_val = from_rust_string(
        env,
        "The network connection was lost. \
         (touchHLE: networking not supported)"
            .to_string(),
    );
    autorelease(env, desc_val);

    let user_info: id = msg_class![env; NSMutableDictionary new];
    autorelease(env, user_info);
    () = msg![env; user_info setObject:desc_val forKey:desc_key];

    let error: id = msg_class![env; NSError alloc];
    let error: id = msg![env;
        error initWithDomain:domain
                        code:NS_URL_ERROR_NOT_CONNECTED_TO_INTERNET
                    userInfo:user_info];
    autorelease(env, error);
    error
}

// ---------------------------------------------------------------------------
// Helper — call `connection:didFailWithError:` on the delegate.
// Uses msg! which already handles unimplemented selectors gracefully.
// ---------------------------------------------------------------------------
fn notify_delegate_failure(env: &mut crate::Environment, connection: id, delegate: id) {
    if delegate == nil {
        return;
    }
    log_dbg!("NSURLConnection: notifying delegate of failure");
    let error = make_network_error(env);
    () = msg![env; delegate connection:connection didFailWithError:error];
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLConnection: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host = Box::new(NSURLConnectionHostObject {
        delegate: nil,
        cancelled: false,
    });
    env.objc.alloc_object(this, host, &mut env.mem)
}

// MARK: - canHandleRequest: (class method)

+ (bool)canHandleRequest:(id)_request {
    // Advertise support so the app doesn't take a different code path;
    // failure is reported via the delegate / error out-param instead.
    true
}

// MARK: - Synchronous API

+ (id)sendSynchronousRequest:(id)request
           returningResponse:(MutPtr<id>)response_ptr
                       error:(MutPtr<id>)error_ptr {

    log!("NSURLConnection sendSynchronousRequest: stub called");

    let bundle_id = env.bundle.bundle_identifier();
    let is_power_rangers = bundle_id.contains("powerrangers") || bundle_id.contains("saban");

    // --- START HACK ---
    // If the request is valid, check if it's our target file
    if request != nil {
        let url: id = msg![env; request URL];
        if url != nil {
            let absolute_url: id = msg![env; url absoluteString];
            let url_str = crate::frameworks::foundation::ns_string::to_rust_string(env, absolute_url);
            
            if url_str.contains("localfeed.xml") {
                log!("HACK: Detected localfeed.xml request string. Generating explicit mock data response.");

                if !response_ptr.is_null() {
                    let mime: id = crate::frameworks::foundation::ns_string::from_rust_string(env, "text/xml".to_string());
                    autorelease(env, mime);
                    let encoding: id = crate::frameworks::foundation::ns_string::from_rust_string(env, "utf-8".to_string());
                    autorelease(env, encoding);

                    // Use standard base NSURLResponse instead of unmapped NSHTTPURLResponse subclass
                    let mock_response: id = msg_class![env; NSURLResponse alloc];
                    let mock_response: id = msg![env; mock_response initWithURL:url 
                                                                       MIMEType:mime 
                                                          expectedContentLength:256 
                                                               textEncodingName:encoding];
                    autorelease(env, mock_response);
                    env.mem.write(response_ptr, mock_response);
                }

                if !error_ptr.is_null() {
                    env.mem.write(error_ptr, nil);
                }

                let xml_str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n    <status>1</status>\n    <success>true</success>\n    <game_enabled>1</game_enabled>\n    <maintenance>0</maintenance>\n</config>".to_string();
                let xml = crate::frameworks::foundation::ns_string::from_rust_string(env, xml_str);
                autorelease(env, xml);

                let data: id = msg![env; xml dataUsingEncoding:4];
                return data;
            }
        }
    }
            
    // Intercept case where request might come across as nil
    if request == nil {
        if is_power_rangers {
            log!("HACK: Caught nil request inside Power Rangers. Injecting safe placeholder URL and data response.");

            if !response_ptr.is_null() {
                let url_str = crate::frameworks::foundation::ns_string::from_rust_string(env, "http://localhost/localfeed.xml".to_string());
                autorelease(env, url_str);
                let fake_url: id = msg_class![env; NSURL URLWithString:url_str];
                autorelease(env, fake_url);

                let mime: id = crate::frameworks::foundation::ns_string::from_rust_string(env, "text/xml".to_string());
                autorelease(env, mime);
                let encoding: id = crate::frameworks::foundation::ns_string::from_rust_string(env, "utf-8".to_string());
                autorelease(env, encoding);

                let mock_response: id = msg_class![env; NSURLResponse alloc];
                let mock_response: id = msg![env; mock_response initWithURL:fake_url 
                                                                   MIMEType:mime 
                                                      expectedContentLength:256 
                                                               textEncodingName:encoding];
                autorelease(env, mock_response);
                env.mem.write(response_ptr, mock_response);
            }

            if !error_ptr.is_null() {
                env.mem.write(error_ptr, nil);
            }

            let xml_str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n    <status>1</status>\n    <success>true</success>\n    <game_enabled>1</game_enabled>\n    <maintenance>0</maintenance>\n</config>".to_string();
            let xml = crate::frameworks::foundation::ns_string::from_rust_string(env, xml_str);
            autorelease(env, xml);

            let data: id = msg![env; xml dataUsingEncoding:4];
            return data;
        }

        log!("NSURLConnection sendSynchronousRequest: nil request — returning empty NSData");

        if !response_ptr.is_null() {
            env.mem.write(response_ptr, nil);
        }

        if !error_ptr.is_null() {
            env.mem.write(error_ptr, nil);
        }

        return msg_class![env; NSData data];
    }
    // --- END HACK ---

    if !response_ptr.is_null() {
        env.mem.write(response_ptr, nil);
    }

    if !error_ptr.is_null() {
        let error = make_network_error(env);
        retain(env, error);
        env.mem.write(error_ptr, error);
    }

    msg_class![env; NSData data]
}
     
// MARK: - Asynchronous API

+ (id)connectionWithRequest:(id)request
                   delegate:(id)delegate {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRequest:request delegate:delegate];
    autorelease(env, new);
    new
}

- (id)initWithRequest:(id)request
             delegate:(id)delegate {
    let initialized_this: id = msg![env; this initWithRequest:request delegate:delegate startImmediately:true];
    initialized_this
}

- (id)initWithRequest:(id)request
             delegate:(id)delegate
     startImmediately:(bool)start_immediately {

    let initialized_this: id = msg![env; this init];
    if initialized_this == nil {
        return nil;
    }

    if request == nil {
        log!("NSURLConnection initWithRequest: nil request handled safely.");
    }

    log!(
        "NSURLConnection initWithRequest:... delegate:... \
         startImmediately:{} (stub — failure via delegate)",
        start_immediately,
    );

    if delegate != nil {
        retain(env, delegate);
        
        let is_valid_connection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            env.objc.borrow_mut::<NSURLConnectionHostObject>(initialized_this);
        })).is_ok();

        if is_valid_connection {
            let mut host = env.objc.borrow_mut::<NSURLConnectionHostObject>(initialized_this);
            host.delegate  = delegate;
            host.cancelled = false;
        } else {
            log!("Warning: initWithRequest called on a non-NSURLConnection generic instance. Binding delegate via KVC fallback.");
            let key = crate::frameworks::foundation::ns_string::get_static_str(env, "delegate");
            () = msg![env; initialized_this setValue:delegate forKey:key];
        }
    }

    if start_immediately {
        log!("NSURLConnection: startImmediately is true, running network completion stub now.");
        () = msg![env; initialized_this start];
    }

    initialized_this
}
      
// MARK: - Instance methods

- (())start {
    log!("NSURLConnection start: Faking graceful network failure (offline mode) to prevent engine panic.");

    let is_valid_connection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.objc.borrow::<NSURLConnectionHostObject>(this);
    })).is_ok();

    let delegate = if is_valid_connection {
        env.objc.borrow::<NSURLConnectionHostObject>(this).delegate
    } else {
        let key = crate::frameworks::foundation::ns_string::get_static_str(env, "delegate");
        let val: id = msg![env; this valueForKey:key];
        val
    };

    if delegate == nil {
        return;
    }

    notify_delegate_failure(env, this, delegate);
}
    
- (())cancel {
    log_dbg!("NSURLConnection cancel");
    let is_valid_connection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.objc.borrow_mut::<NSURLConnectionHostObject>(this);
    })).is_ok();

    if is_valid_connection {
        env.objc.borrow_mut::<NSURLConnectionHostObject>(this).cancelled = true;
    }
}

// MARK: - Dealloc

- (())dealloc {
    log_dbg!("NSURLConnection dealloc");
    let is_valid_connection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.objc.borrow::<NSURLConnectionHostObject>(this);
    })).is_ok();

    if is_valid_connection {
        let delegate = env.objc.borrow::<NSURLConnectionHostObject>(this).delegate;
        release(env, delegate);
    }
    env.objc.dealloc_object(this, &mut env.mem);
}

@end

};
