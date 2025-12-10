//! This module contains the [`clear_with`] function that works as `drain(..)` but faster.

/// This function is like [`drain`](Vec::drain) for the whole [`Vec`], but faster.
pub fn clear_with<T, F>(vec: &mut alloc::vec::Vec<T>, mut f: F)
where
    F: FnMut(T),
{
    for item in vec.iter_mut() {
        unsafe { f(core::ptr::read(item)) };
    }

    unsafe { vec.set_len(0) };
}
