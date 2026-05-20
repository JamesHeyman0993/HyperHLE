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
    // 1. Check if a context exists using an immutable borrow first to appease the borrow checker.
    let has_context = state.current_ctxs.get(&current_thread).and_then(|c| *c).is_some();

    // 2. If it doesn't exist, scan for a fallback context before doing any mutable work.
    if !has_context {
        log!("Warning: get_thread_context called on a thread with no active EAGLContext bound. Attempting fallback.");
        
        let fallback_id = state.current_ctxs.values()
            .find_map(|&opt| opt) // Finds the first Some(id)
            .raw_unwrap_or_else(|| {
                panic!("Fatal Error: Context lookup failed. The app attempted to perform GL operations before initializing any EAGLContext.");
            });

        log!("Found fallback context ID: {:?}", fallback_id);
        
        // Mutably assign the fallback safely now that all immutable scans are finished.
        *state.current_ctx_for_thread(current_thread) = Some(fallback_id);
    }

    // 3. We are guaranteed to have a context mapped now. Get it mutably.
    let context_id = state.current_ctx_for_thread(current_thread).unwrap();
    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(context_id);
    
    if host_obj.gles_ctx.is_none() {
        log!("Warning: get_thread_context found an uninitialized context. Initializing GLES2NativeContext on demand.");
        
        match crate::gles::gles2_native::GLES2NativeContext::new(window) {
            Ok(ctx) => {
                host_obj.gles_ctx = Some(Box::new(ctx));
            }
            Err(e) => {
                panic!("Failed to late-initialize GLES2NativeContext for worker thread: {}", e);
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
