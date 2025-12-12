//! This module contains the [`ArrayBuffer`].
use crate::hints::{assert_hint, likely, unlikely};
use core::mem;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

/// `ArrayBuffer` is a fixed-sized array-based buffer.
///
/// # Example
///
/// ```rust
/// use std::mem::MaybeUninit;
/// use orengine_utils::ArrayBuffer;
///
/// let mut buffer = ArrayBuffer::<u16, 4>::new();
///
/// unsafe {
///     buffer.refill_with(|buf| {
///         buf[0..2].copy_from_slice(&[MaybeUninit::new(22), MaybeUninit::new(23)]);
///
///         2
///     });
/// }
///
/// buffer[1] = 21;
///
/// assert_eq!(buffer.pop(), Some(21));
/// assert_eq!(buffer.pop(), Some(22));
/// ```
pub struct ArrayBuffer<T, const N: usize> {
    array: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> ArrayBuffer<T, N> {
    /// Creates a new ` ArrayBuffer `.
    pub fn new() -> Self {
        #[allow(
            clippy::uninit_assumed_init,
            reason = "We guarantee that the array is initialized, when reading from it"
        )]
        {
            Self {
                array: [const { MaybeUninit::uninit() }; N],
                len: 0,
            }
        }
    }

    /// Returns the capacity of the buffer.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a pointer to the first element of the buffer.
    pub const fn as_ptr(&self) -> *const T {
        self.array.as_ptr().cast()
    }

    /// Returns a mutable pointer to the first element of the buffer.
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.array.as_mut_ptr().cast()
    }

    /// Appends an element to the buffer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the buffer is not full.
    pub unsafe fn push_unchecked(&mut self, item: T) {
        assert_hint(self.len() < N, "Tried to push to a full array buffer");

        self.array[self.len].write(item);
        self.len += 1;
    }

    /// Appends an element to the buffer or returns `Err(value)` if the buffer is full.
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if unlikely(self.len == self.capacity()) {
            return Err(item);
        }

        unsafe { self.push_unchecked(item) };

        Ok(())
    }

    /// Pops an element from the buffer or returns `None` if the buffer is empty.
    pub fn pop(&mut self) -> Option<T> {
        if unlikely(self.len == 0) {
            return None;
        }

        self.len -= 1;

        Some(unsafe { self.array[self.len].as_ptr().read() })
    }

    /// Clears with calling the provided function on each element.
    pub fn clear_with<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        for i in 0..self.len {
            f(unsafe { self.array[i].as_ptr().read() });
        }

        self.len = 0;
    }

    /// Drops all elements in the buffer and set the length to 0.
    pub fn clear(&mut self) {
        if mem::needs_drop::<T>() {
            for i in 0..self.len {
                drop(unsafe { self.array[i].as_ptr().read() });
            }
        }

        self.len = 0;
    }

    /// Returns a reference iterator over the buffer.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        struct Iter<'array_buffer, T, const N: usize> {
            buffer: &'array_buffer ArrayBuffer<T, N>,
            current: *const T,
            end: *const T,
        }

        impl<'array_buffer, T, const N: usize> Iterator for Iter<'array_buffer, T, N> {
            type Item = &'array_buffer T;

            fn next(&mut self) -> Option<Self::Item> {
                if likely(self.current < self.end) {
                    let item = unsafe { &*self.current };

                    unsafe {
                        self.current = self.current.add(1);
                    }

                    Some(item)
                } else {
                    None
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let size = (self.end as usize - self.current as usize) / size_of::<T>();

                (size, Some(size))
            }
        }

        impl<T, const N: usize> ExactSizeIterator for Iter<'_, T, N> {
            fn len(&self) -> usize {
                self.buffer.len
            }
        }

        let current = (&raw const self.array[0]).cast();

        Iter {
            buffer: self,
            current,
            end: unsafe { current.add(self.len) },
        }
    }

    /// Returns a mutable reference iterator over the buffer.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut T> {
        struct IterMut<'array_buffer, T, const N: usize> {
            buffer: &'array_buffer mut ArrayBuffer<T, N>,
            current: *mut T,
            end: *mut T,
        }

        impl<'array_buffer, T, const N: usize> Iterator for IterMut<'array_buffer, T, N> {
            type Item = &'array_buffer mut T;

            fn next(&mut self) -> Option<Self::Item> {
                if likely(self.current < self.end) {
                    let item = unsafe { &mut *self.current };

                    unsafe {
                        self.current = self.current.add(1);
                    }

                    Some(item)
                } else {
                    None
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let size = (self.end as usize - self.current as usize) / size_of::<T>();

                (size, Some(size))
            }
        }

        impl<T, const N: usize> ExactSizeIterator for IterMut<'_, T, N> {
            fn len(&self) -> usize {
                self.buffer.len
            }
        }

        let current: *mut T = (&raw mut self.array[0]).cast();
        let end = unsafe { current.add(self.len) };

        IterMut {
            buffer: self,
            current,
            end,
        }
    }

    /// Refills the buffer with elements provided by the function.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the buffer is empty before refilling.
    pub unsafe fn refill_with(&mut self, f: impl FnOnce(&mut [MaybeUninit<T>; N]) -> usize) {
        debug_assert!(
            self.is_empty(),
            "ArrayBuffer should be empty before refilling"
        );

        let filled = f(&mut self.array);

        debug_assert!(filled <= N, "Filled more than the capacity");

        self.len = filled;
    }
    /// Returns a pointer to the underlying array.
    fn as_slice_ptr(&self) -> *const [T] {
        slice_from_raw_parts(self.as_ptr(), self.len)
    }

    /// Returns a mutable pointer to the underlying array.
    fn as_mut_slice_ptr(&mut self) -> *mut [T] {
        slice_from_raw_parts_mut(self.as_mut_ptr(), self.len)
    }
}

impl<T, const N: usize> Deref for ArrayBuffer<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.as_slice_ptr() }
    }
}

impl<T, const N: usize> AsRef<[T]> for ArrayBuffer<T, N> {
    fn as_ref(&self) -> &[T] {
        unsafe { &*self.as_slice_ptr() }
    }
}

impl<T, const N: usize> DerefMut for ArrayBuffer<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.as_mut_slice_ptr() }
    }
}

impl<T, const N: usize> AsMut<[T]> for ArrayBuffer<T, N> {
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { &mut *self.as_mut_slice_ptr() }
    }
}

impl<T, const N: usize> Default for ArrayBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> From<[T; N]> for ArrayBuffer<T, N> {
    fn from(array: [T; N]) -> Self {
        Self {
            array: unsafe { (&raw const array).cast::<[MaybeUninit<T>; N]>().read() },
            len: N,
        }
    }
}

impl<T, const N: usize> Drop for ArrayBuffer<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[allow(
        clippy::explicit_auto_deref,
        reason = "We test deref and deref_mut methods"
    )]
    #[test]
    fn test_array_buffer_pop_push_len() {
        let mut buffer = ArrayBuffer::<u32, 4>::new();

        unsafe {
            buffer.push_unchecked(1);
            assert_eq!(buffer.len(), 1);
            assert_eq!((*buffer).len(), 1);

            buffer.push_unchecked(2);
            assert_eq!(buffer.len(), 2);
            assert_eq!((*buffer).len(), 2);

            buffer.push(3).unwrap();
            assert_eq!(buffer.len(), 3);
            assert_eq!(buffer.as_ref().len(), 3);

            assert_eq!(buffer.pop(), Some(3));
            assert_eq!(buffer.len(), 2);
            assert_eq!(buffer.as_mut().len(), 2);

            buffer.push_unchecked(4);
            assert_eq!(buffer.len(), 3);
            assert_eq!(buffer.deref_mut().len(), 3);

            buffer.push_unchecked(5);
            assert_eq!(buffer.len(), 4);
            assert_eq!(buffer.deref_mut().len(), 4);

            assert_eq!(buffer.push(6), Err(6));

            assert_eq!(buffer.pop(), Some(5));
            assert_eq!(buffer.pop(), Some(4));
            assert_eq!(buffer.pop(), Some(2));
            assert_eq!(buffer.pop(), Some(1));
            assert_eq!(buffer.pop(), None);
        }
    }

    #[test]
    fn test_array_buffer_iterators() {
        let mut buffer = ArrayBuffer::<u32, 4>::new();

        unsafe {
            buffer.push_unchecked(1);
            buffer.push_unchecked(2);
            buffer.push_unchecked(3);
            buffer.push_unchecked(4);
        }

        assert_eq!(buffer.iter().collect::<Vec<_>>(), vec![&1, &2, &3, &4]);
        assert_eq!(
            buffer.iter_mut().collect::<Vec<_>>(),
            vec![&mut 1, &mut 2, &mut 3, &mut 4]
        );
    }

    #[test]
    fn test_array_buffer_refill_with() {
        let mut buffer = ArrayBuffer::<u32, 4>::new();

        unsafe {
            buffer.refill_with(|array| {
                array.copy_from_slice(&[
                    MaybeUninit::new(1),
                    MaybeUninit::new(2),
                    MaybeUninit::new(3),
                    MaybeUninit::new(4),
                ]);

                4
            });
        };

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.iter().collect::<Vec<_>>(), vec![&1, &2, &3, &4]);
    }
}
