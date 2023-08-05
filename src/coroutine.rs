//! Coroutine structures,
//! allowing you to run async code without using futures.
//!
//! Useful for loading assets

use std::{
    cell::Cell,
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
    task::{Poll, Waker},
};

use crate::Context;

enum CoroutineState<T> {
    Future(Pin<Box<dyn Future<Output = T> + 'static>>),
    Finished,
}

// TODO custom context
// TODO
// TODO let coroutines has specific parameters?
// TODO as in `Coroutine<Filesystem, Vec<u8>> has poll(Filesystem) -> Option<Vec<u8>>`

/// Coroutine structure
#[allow(missing_debug_implementations)]
pub struct Coroutine<T = ()> {
    waker: Waker,
    ctx_holder: UnsafeHolder<Context>,
    state: CoroutineState<T>,
}

impl<T> Coroutine<T> {
    /// Constructs a new coroutine
    pub fn new<F: Future<Output = T> + 'static>(
        fut: impl FnOnce(UnsafeHolder<Context>) -> F,
    ) -> Self {
        struct Inner;
        impl std::task::Wake for Inner {
            fn wake(self: Arc<Self>) {}
        }

        let waker = Waker::from(Arc::new(Inner));
        let ctx_holder = UnsafeHolder(Arc::new(Cell::new(std::ptr::null_mut())));
        let fut = fut(UnsafeHolder(Arc::clone(&ctx_holder.0)));

        Self {
            waker,
            state: CoroutineState::Future(Box::pin(fut)),
            ctx_holder,
        }
    }

    /// Advances and possibly returns a value from the coroutine.
    pub fn poll(&mut self, ctx: &mut Context) -> Option<T> {
        match &mut self.state {
            // If the future isn't done, poll it
            CoroutineState::Future(fut) => {
                let mut task_context = std::task::Context::from_waker(&self.waker);
                self.ctx_holder.0.set(ctx as *mut Context);
                let result = match fut.as_mut().poll(&mut task_context) {
                    // If the future finished, return the value and set the coroutine to a finished state
                    Poll::Ready(v) => {
                        self.state = CoroutineState::Finished;
                        Some(v)
                    }
                    Poll::Pending => None,
                };
                self.ctx_holder.0.set(std::ptr::null_mut());
                result
            }
            CoroutineState::Finished => None,
        }
    }
}

// Safety: Can't be constructed outside of this module, so usage can be controlled.
/// This can probably still be misused and cause UB so please use it correctly.
#[derive(Debug)]
pub struct UnsafeHolder<T>(Arc<Cell<*mut T>>);

impl<T> UnsafeHolder<T> {
    #[allow(unsafe_code)]
    /// Gets the internal pointer.
    pub fn get_ptr(&self) -> *mut T {
        let val = self.0.get();
        if val.is_null() {
            panic!("Accessed UnsafeHolder when it shouldn't be accessed.")
        }
        val
    }
}

impl<T> Deref for UnsafeHolder<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        #[allow(unsafe_code)]
        unsafe {
            &*self.get_ptr()
        }
    }
}

impl<T> DerefMut for UnsafeHolder<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[allow(unsafe_code)]
        unsafe {
            &mut *self.get_ptr()
        }
    }
}

struct YieldOp(bool);

impl Future for YieldOp {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if !self.0 {
            self.0 = true;
            return std::task::Poll::Pending;
        }

        std::task::Poll::Ready(())
    }
}

/// Wait 1 poll before finishing.
/// Useful for making infinite coroutines without blocking [`Coroutine::poll`] forever
pub fn yield_now() -> impl Future<Output = ()> {
    YieldOp(false)
}
