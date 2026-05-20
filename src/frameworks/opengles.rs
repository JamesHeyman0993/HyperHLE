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
    /// Emergency fallback context to handle cases where the app triggers GL operations
    /// before registering an active context with the runtime.
    emergency_ctx: Option<Box<dyn crate::gles::GLESContext>>,
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

    // 2. If a valid context exists, run the normal extraction logic
    if has_context {
        let context_id = state.current_ctx_for_thread(current_thread).unwrap();
        let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(context_id);
        
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
        return host_obj.gles_ctx.as_deref_mut().unwrap();
    }

    // 3. Sibling Fallback Search: If this thread lacks a context, check if another thread has one
    if let Some(context_id) = state.current_ctxs.values().find_map(|&opt| opt) {
        log!("Found active sibling context ID fallback: {:?}", context_id);
        *state.current_ctx_for_thread(current_thread) = Some(context_id);
        
        let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(context_id);
        if host_obj.gles_ctx.is_none() {
            match crate::gles::gles2_native::GLES2NativeContext::new(window) {
                Ok(ctx) => host_obj.gles_ctx = Some(Box::new(ctx)),
                Err(e) => panic!("Failed to late-initialize GLES2NativeContext fallback: {}", e),
            }
        }
        return host_obj.gles_ctx.as_deref_mut().unwrap();
    }

    // 4. Emergency Recovery: No contexts exist anywhere. Fall back to our structural emergency context instance.
    log!("Warning: No context mappings exist anywhere. Utilizing module emergency fallback context.");
    if state.emergency_ctx.is_none() {
        match crate::gles::gles2_native::GLES2NativeContext::new(window) {
            Ok(ctx) => {
                state.emergency_ctx = Some(Box::new(ctx));
            }
            Err(e) => {
                panic!("Failed to instantiate module emergency fallback context: {}", e);
            }
        }
    }

    state.emergency_ctx.as_deref_mut().unwrap()
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
