//! This module provides [`LightArc`].
use crate::hints::unlikely;
use alloc::alloc::{dealloc, Layout};
use alloc::boxed::Box;
use core::ptr;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// An inner type for [`LightArc`].
#[repr(C)]
struct LightArcInner<T> {
    ref_count: AtomicUsize,
    value: T,
}

/// A light-weight reference-counted pointer to a value.
///
/// This is similar to [`Arc`](std::sync::Arc), but is more lightweight because it contains only a strong count.
/// Therefore, it can't provide weak references.
#[repr(C)]
pub struct LightArc<T> {
    inner: NonNull<LightArcInner<T>>,
}

impl<T> LightArc<T> {
    /// Creates a new [`LightArc`] from the given value.
    pub fn new(value: T) -> Self {
        let inner = Box::new(LightArcInner {
            ref_count: AtomicUsize::new(1),
            value,
        });

        Self {
            inner: NonNull::from(Box::leak(inner)),
        }
    }

    /// Returns a reference to the inner value.
    fn inner(&self) -> &LightArcInner<T> {
        unsafe { self.inner.as_ref() }
    }

    /// Drops and deallocates the inner value.
    ///
    /// # Safety
    ///
    /// This function must only be called when the reference count is 0.
    #[inline(never)]
    unsafe fn drop_slow(&mut self) {
        core::sync::atomic::fence(Ordering::Acquire);

        unsafe {
            ptr::drop_in_place(&raw mut self.inner.as_mut().value);

            dealloc(
                self.inner.as_ptr().cast(),
                Layout::new::<LightArcInner<T>>(),
            );
        }
    }
}

impl<T> Clone for LightArc<T> {
    fn clone(&self) -> Self {
        let count = self.inner().ref_count.fetch_add(1, Ordering::Relaxed);

        debug_assert!(count > 0, "use after free");

        Self { inner: self.inner }
    }
}

impl<T> Drop for LightArc<T> {
    fn drop(&mut self) {
        if unlikely(self.inner().ref_count.fetch_sub(1, Ordering::Release) == 1) {
            unsafe { self.drop_slow() };
        }
    }
}

impl<T> core::ops::Deref for LightArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

unsafe impl<T: Send + Sync> Send for LightArc<T> {}
unsafe impl<T: Send + Sync> Sync for LightArc<T> {}
