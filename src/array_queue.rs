//! This module contains the [`ArrayQueue`].
use crate::hints::{assert_hint, likely, unlikely};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::{mem, ptr};

/// `ArrayQueue` is a queue, but it uses an array on a stack and can't be resized.
///
/// # Example
///
/// ```rust
/// use orengine_utils::ArrayQueue;
///
/// let mut queue = ArrayQueue::<u32, 2>::new();
///
/// queue.push(1).unwrap();
/// queue.push(2).unwrap();
///
/// assert_eq!(queue.pop(), Some(1));
/// assert_eq!(queue.pop(), Some(2));
/// assert_eq!(queue.pop(), None);
/// ```
pub struct ArrayQueue<T, const N: usize> {
    array: ManuallyDrop<[T; N]>,
    len: usize,
    head: usize,
}

impl<T, const N: usize> ArrayQueue<T, N> {
    /// Creates new `ArrayQueue`.
    pub fn new() -> Self {
        #[allow(
            clippy::uninit_assumed_init,
            reason = "We guarantee that the array is initialized, when reading from it"
        )]
        {
            Self {
                array: ManuallyDrop::new(unsafe { MaybeUninit::uninit().assume_init() }),
                len: 0,
                head: 0,
            }
        }
    }

    /// Returns the capacity of the queue.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the number of elements in the queue.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns an index of the underlying array for the provided index.
    #[inline]
    fn to_physical_idx(&self, idx: usize) -> usize {
        let logical_index = self.head + idx;

        debug_assert!(logical_index < N || (logical_index - N) < N);

        if unlikely(logical_index >= N) {
            logical_index - N
        } else {
            logical_index
        }
    }

    /// Appends an element to the back of the queue.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the stack is not full.
    pub unsafe fn push_unchecked(&mut self, value: T) {
        assert_hint(self.len() < N, "Tried to push to a full array stack");

        let idx = self.to_physical_idx(self.len());

        unsafe { ptr::write(self.array.get_unchecked_mut(idx), value) };

        self.len += 1;
    }

    /// Appends an element to the back of the queue or returns `Err(value)` if the queue is full.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if likely(self.len() < N) {
            unsafe { self.push_unchecked(value) };

            Ok(())
        } else {
            Err(value)
        }
    }

    /// Removes the first element and returns it, or `None` if the queue is empty.
    pub fn pop(&mut self) -> Option<T> {
        if !self.is_empty() {
            self.len -= 1;

            let idx = self.head;
            self.head = self.to_physical_idx(1);

            assert_hint(
                self.array.len() > idx,
                &format!("idx: {}, len: {}", idx, self.array.len()),
            );

            Some(unsafe { ptr::read(self.array.get_unchecked_mut(idx)) })
        } else {
            None
        }
    }

    /// Drops all elements in the queue and set the length to 0.
    pub fn clear(&mut self) {
        if mem::needs_drop::<T>() {
            for i in 0..self.len {
                let idx = self.to_physical_idx(i);

                unsafe { ptr::drop_in_place(self.array.get_unchecked_mut(idx)) };
            }
        }

        self.len = 0;
    }

    /// Clears with calling the provided function on each element.
    pub fn clear_with<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        for i in 0..self.len {
            let idx = self.to_physical_idx(i);

            let value = unsafe { ptr::read(self.array.get_unchecked_mut(idx)) };

            f(value);
        }

        self.len = 0;
    }

    /// Returns a reference iterator over the queue.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        struct Iter<'array_queue, T, const N: usize> {
            queue: &'array_queue ArrayQueue<T, N>,
            iterated: usize,
        }

        impl<'array_queue, T, const N: usize> Iterator for Iter<'array_queue, T, N> {
            type Item = &'array_queue T;

            fn next(&mut self) -> Option<Self::Item> {
                if self.iterated < self.queue.len {
                    let idx = self.queue.to_physical_idx(self.iterated);

                    self.iterated += 1;

                    Some(unsafe { self.queue.array.get_unchecked(idx) })
                } else {
                    None
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.queue.len - self.iterated, Some(self.queue.len - self.iterated))
            }
        }

        impl<T, const N: usize> ExactSizeIterator for Iter<'_, T, N> {
            fn len(&self) -> usize {
                self.queue.len
            }
        }

        Iter {
            queue: self,
            iterated: 0,
        }
    }

    /// Returns a mutable reference iterator over the queue.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut T> {
        struct IterMut<'array_queue, T, const N: usize> {
            queue: &'array_queue mut ArrayQueue<T, N>,
            iterated: usize,
        }

        impl<'array_queue, T, const N: usize> Iterator for IterMut<'array_queue, T, N> {
            type Item = &'array_queue mut T;

            fn next(&mut self) -> Option<Self::Item> {
                if self.iterated < self.queue.len {
                    let idx = self.queue.to_physical_idx(self.iterated);

                    self.iterated += 1;

                    Some(unsafe { &mut *(self.queue.array.get_unchecked_mut(idx) as *mut T) })
                } else {
                    None
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.queue.len - self.iterated, Some(self.queue.len - self.iterated))
            }
        }

        impl<T, const N: usize> ExactSizeIterator for IterMut<'_, T, N> {
            fn len(&self) -> usize {
                self.queue.len
            }
        }

        IterMut {
            queue: self,
            iterated: 0,
        }
    }

    /// Refills the queue with elements provided by the function.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the queue is empty before refilling.
    pub unsafe fn refill_with(&mut self, f: impl FnOnce(&mut [T; N]) -> usize) {
        debug_assert!(self.is_empty(), "ArrayQueue should be empty before refilling");

        let filled = f(&mut self.array);

        self.len = filled;
        self.head = 0;
    }
}

impl<T, const N: usize> Deref for ArrayQueue<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &*self.array
    }
}

impl<T, const N: usize> AsRef<[T]> for ArrayQueue<T, N> {
    fn as_ref(&self) -> &[T] {
        &*self.array
    }
}

impl<T, const N: usize> DerefMut for ArrayQueue<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.array
    }
}

impl<T, const N: usize> AsMut<[T]> for ArrayQueue<T, N> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut *self.array
    }
}

impl<T, const N: usize> Default for ArrayQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> From<[T; N]> for ArrayQueue<T, N> {
    fn from(array: [T; N]) -> Self {
        Self {
            array: ManuallyDrop::new(array),
            len: N,
            head: 0,
        }
    }
}

impl<T, const N: usize> Drop for ArrayQueue<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_queue() {
        let mut queue = ArrayQueue::<u32, 4>::new();

        unsafe {
            queue.push_unchecked(1);
            assert_eq!(queue.len(), 1);

            queue.push_unchecked(2);
            assert_eq!(queue.len(), 2);

            queue.push(3).unwrap();
            assert_eq!(queue.len(), 3);

            assert_eq!(queue.pop(), Some(1));
            assert_eq!(queue.len(), 2);

            queue.push_unchecked(4);
            assert_eq!(queue.len(), 3);

            queue.push_unchecked(5);
            assert_eq!(queue.len(), 4);

            assert_eq!(queue.push(6), Err(6));

            assert_eq!(queue.pop(), Some(2));
            assert_eq!(queue.pop(), Some(3));
            assert_eq!(queue.pop(), Some(4));
            assert_eq!(queue.pop(), Some(5));
            assert_eq!(queue.pop(), None);
        }
    }

    #[test]
    fn test_array_queue_iterators() {
        let mut queue = ArrayQueue::<u32, 4>::new();

        unsafe {
            queue.push_unchecked(1);
            queue.push_unchecked(2);
            queue.push_unchecked(3);
            queue.push_unchecked(4);
        }

        assert_eq!(queue.iter().collect::<Vec<_>>(), vec![&1, &2, &3, &4]);
        assert_eq!(queue.iter_mut().collect::<Vec<_>>(), vec![&mut 1, &mut 2, &mut 3, &mut 4]);
    }

    #[test]
    fn test_array_queue_refill_with() {
        let mut queue = ArrayQueue::<u32, 4>::new();

        unsafe {
        queue.refill_with(|array| {
                array.copy_from_slice(&[1, 2, 3, 4]);
                4
            });
        };

        assert_eq!(queue.len(), 4);
        assert_eq!(queue.iter().collect::<Vec<_>>(), vec![&1, &2, &3, &4]);
    }
}