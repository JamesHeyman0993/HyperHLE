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

// Bring the context trait into scope so that `new()` is available on implementations
use crate::gles::gles_generic::GLESContext;

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
    // Made pub(crate) so it is cleanly accessible inside submodules like eagl.rs
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
    // Pass window down so we can initialize the context if needed
    let gles_ctx = get_thread_context(state, objc, window, current_thread);
    gles_ctx.make_current(window)
}

// Added pub(crate) so eagl.rs can access this utility directly
pub(crate) fn get_thread_context<'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    window: &mut crate::window::Window,
    current_thread: crate::ThreadId,
) -> &'objc mut dyn crate::gles::GLESContext {
    let current_ctx_option = state.current_ctx_for_thread(current_thread);
    
    // Safety check: Avoid calling .unwrap() blindly. If a background worker thread
    // executes a GL invocation before setting its current context, attempt to fall back
    // to any active context to prevent an immediate game crash.
    let context_id = match *current_ctx_option {
        Some(id) => id,
        None => {
            log!("Warning: get_thread_context called on a thread with no active EAGLContext bound. Attempting fallback.");
            if let Some(Some(fallback_id)) = state.current_ctxs.values().find(|c| c.is_some()) {
                log!("Found fallback context ID: {:?}", fallback_id);
                *current_ctx_option = Some(*fallback_id);
                *fallback_id
            } else {
                panic!("Fatal Error: Context lookup failed. The app attempted to perform GL operations before initializing any EAGLContext.");
            }
        }
    };

    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(context_id);
    
    // If the underlying host GLES context hasn't been initialized yet, do it now!
    if host_obj.gles_ctx.is_none() {
        log!("Warning: get_thread_context found an uninitialized context. Initializing GLES2NativeContext on demand.");
        
        // Routed via explicit submodule path to bypass the private visibility restriction
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
