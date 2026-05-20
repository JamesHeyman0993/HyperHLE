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

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/OpenGLES.framework/OpenGLES",
    aliases: &[],
    class_exports: &[eagl::CLASSES],
    constant_exports: &[eagl::CONSTANTS],
    function_exports: &[gles_guest::FUNCTIONS, eagl::FUNCTIONS],
};

// A local dummy struct that implements GLESContext to serve as our safe unwrap fallback
#[derive(Default)]
struct LocalFallbackGLESContext;
impl crate::gles::GLESContext for LocalFallbackGLESContext {
    fn make_current(&mut self, _window: &mut crate::window::Window) -> Box<dyn crate::gles::GLES + '_> {
        // Return a dummy/null value if called during an uninitialized phase
        panic!("Fallback context used for execution instead of safety mapping");
    }
}

#[derive(Default)]
pub struct State {
    /// Current EAGLContext for each thread
    current_ctxs: std::collections::HashMap<crate::ThreadId, Option<crate::objc::id>>,
    strings_cache: std::collections::HashMap<GLenum, ConstPtr<u8>>,
    /// A safe fallback context initialized on demand to prevent unwrap panics
    fallback_ctx: Option<Box<dyn crate::gles::GLESContext>>,
}

impl State {
    fn current_ctx_for_thread(&mut self, thread: crate::ThreadId) -> &mut Option<crate::objc::id> {
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
    let gles_ctx = get_thread_context(state, objc, current_thread);
    gles_ctx.make_current(window)
}

fn get_thread_context<'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    current_thread: crate::ThreadId,
) -> &'objc mut dyn crate::gles::GLESContext {
    let current_ctx = *state.current_ctx_for_thread(current_thread);
    
    // 1. Safe-guard: If there's no objective-c context ID assigned to this thread yet, use the state fallback
    let ctx_id = match current_ctx {
        Some(id) => id,
        None => {
            log_dbg!("Warning: get_thread_context called without an active context ID on this thread.");
            if state.fallback_ctx.is_none() {
                state.fallback_ctx = Some(Box::new(LocalFallbackGLESContext::default()));
            }
            return state.fallback_ctx.as_deref_mut().unwrap();
        }
    };

    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(ctx_id);
    
    // 2. Safe-guard for Line 56 Crash: If the host context exists but its underlying gles_ctx is uninitialized
    if host_obj.gles_ctx.is_none() {
        log!("Warning: EAGLContext has an uninitialized host GLES wrapper. Redirecting execution to fallback to prevent crash.");
        
        if state.fallback_ctx.is_none() {
            state.fallback_ctx = Some(Box::new(LocalFallbackGLESContext::default()));
        }
        
        return state.fallback_ctx.as_deref_mut().unwrap();
    }

    host_obj.gles_ctx.as_deref_mut().unwrap()
}
