/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! OpenGL ES and EAGL.
//!
//! This module is specific to OpenGL ES's role as a part of the iPhone OS API
//! surface. See [crate::gles] for other uses and a discussion of the broader
//! topic.

mod eagl;
mod gles_guest;

use touchHLE_gl_bindings::gles11::types::GLenum;
use crate::mem::ConstPtr;
use crate::gles::GLESContext;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/OpenGLES.framework/OpenGLES",
    aliases: &[],
    class_exports: &[eagl::CLASSES],
    constant_exports: &[eagl::CONSTANTS],
    function_exports: &[gles_guest::FUNCTIONS, eagl::FUNCTIONS],
};

#[derive(Default)]
pub struct State {
    /// Current EAGLContext for each thread
    current_ctxs: std::collections::HashMap<crate::ThreadId, Option<crate::objc::id>>,
    strings_cache: std::collections::HashMap<GLenum, ConstPtr<u8>>,
}

impl State {
    pub(crate) fn current_ctx_for_thread(&mut self, thread: crate::ThreadId) -> &mut Option<crate::objc::id> {
        self.current_ctxs.entry(thread).or_insert(None);
        self.current_ctxs.get_mut(&thread).unwrap()
    }
}

fn sync_context<'objc, 'win: 'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    window: &'win mut crate::window::Window,
    current_thread: crate::ThreadId,
) -> Box<dyn crate::gles::GLES + 'objc> {
    let gles_ctx = get_thread_context(state, objc, window, current_thread);
    gles_ctx.make_current(window)
}

pub(crate) fn get_thread_context<'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    window: &mut crate::window::Window,
    current_thread: crate::ThreadId,
) -> &'objc mut dyn crate::gles::GLESContext {
    // 1. Check if a valid context is already mapped to this thread
    let has_context = state.current_ctxs.get(&current_thread).and_then(|c| *c).is_some();

    // 2. Resolve missing context states safely without panicking
    if !has_context {
        log!("Warning: get_thread_context called on a thread with no active EAGLContext bound. Attempting recovery.");
        
        // Scan to see if ANY other thread has an active context we can borrow
        let existing_fallback = state.current_ctxs.values().find_map(|&opt| opt);

        let context_id = match existing_fallback {
            Some(id) => {
                log!("Found active sibling context ID fallback: {:?}", id);
                id
            }
            None => {
                log!("No contexts exist anywhere in the environment. Allocating an emergency global default EAGLContext.");
                
                // Construct a raw proxy EAGLContext instance to allocate state structure
                let new_context_id = objc.alloc_and_init::<eagl::EAGLContextHostObject>();
                
                // Immediately map the emergency proxy to this thread
                *state.current_ctx_for_thread(current_thread) = Some(new_context_id);
                new_context_id
            }
        };

        // Ensure this thread is mapped to our target context ID
        *state.current_ctx_for_thread(current_thread) = Some(context_id);
    }

    // 3. Extract the context mutably
    let context_id = state.current_ctx_for_thread(current_thread).unwrap();
    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(context_id);
    
    // 4. On-demand initialization of the underlying hardware layer
    if host_obj.gles_ctx.is_none() {
        log!("Warning: get_thread_context initializing underlying GLES2NativeContext backend layer.");
        
        match crate::gles::gles2_native::GLES2NativeContext::new(window) {
            Ok(ctx) => {
                host_obj.gles_ctx = Some(Box::new(ctx));
            }
            Err(e) => {
                panic!("Failed to late-initialize GLES2NativeContext backend: {}", e);
            }
        }
    }

    host_obj.gles_ctx.as_deref_mut().unwrap()
}

// Simple extension helper trait to provide raw unwrapping capabilities
trait OptionalExt<T> {
    fn raw_unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T;
}
impl<T> OptionalExt<T> for Option<T> {
    fn raw_unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Some(val) => val,
            None => f(),
        }
    }
}
