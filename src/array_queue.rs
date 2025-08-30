//! This module contains the [`ArrayQueue`].
use crate::hints::{assert_hint, likely, unlikely};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::{mem, ptr};
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

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
    array: [MaybeUninit<T>; N],
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
                array: [const { MaybeUninit::uninit() }; N],
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
    fn to_physical_idx_from_head(&self, idx: usize) -> usize {
        let logical_index = self.head + idx;

        debug_assert!(logical_index < N || (logical_index - N) < N);

        if unlikely(logical_index >= N) {
            logical_index - N
        } else {
            logical_index
        }
    }

    /// Returns a pair of slices that represent the occupied region of the queue.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut array_queue = ArrayQueue::<u32, 4>::new();
    ///
    /// array_queue.push(1).unwrap();
    /// array_queue.push(2).unwrap();
    ///
    /// let should_be: (&[u32], &[u32]) = (&[1, 2], &[]);
    ///
    /// assert_eq!(array_queue.as_slices(), should_be);
    ///
    /// array_queue.push(3).unwrap();
    /// array_queue.push(4).unwrap();
    ///
    /// assert_eq!(array_queue.pop(), Some(1));
    /// assert_eq!(array_queue.pop(), Some(2));
    ///
    /// array_queue.push(5).unwrap();
    ///
    /// let should_be: (&[u32], &[u32]) = (&[3, 4], &[5]);
    ///
    /// assert_eq!(array_queue.as_slices(), should_be);
    /// ```
    pub fn as_slices(&self) -> (&[T], &[T]) {
        let phys_head = self.to_physical_idx_from_head(0);
        let phys_tail = self.to_physical_idx_from_head(self.len());

        if phys_tail > phys_head {
            (
                unsafe { &*slice_from_raw_parts(self.array.as_ptr().add(phys_head).cast(), self.len) },
                &[],
            )
        } else {
            (
                unsafe { &*slice_from_raw_parts(self.array.as_ptr().add(phys_head).cast(), N - phys_head) },
                unsafe { &*slice_from_raw_parts(self.array.as_ptr().cast(), phys_tail) },
            )
        }
    }

    /// Returns a pair of mutable slices that represent the occupied region of the queue.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut array_queue = ArrayQueue::<u32, 4>::new();
    ///
    /// array_queue.push(1).unwrap();
    /// array_queue.push(2).unwrap();
    ///
    /// let should_be: (&mut [u32], &mut [u32]) = (&mut [1, 2], &mut []);
    ///
    /// assert_eq!(array_queue.as_mut_slices(), should_be);
    ///
    /// array_queue.push(3).unwrap();
    /// array_queue.push(4).unwrap();
    ///
    /// assert_eq!(array_queue.pop(), Some(1));
    /// assert_eq!(array_queue.pop(), Some(2));
    ///
    /// array_queue.push(5).unwrap();
    ///
    /// let should_be: (&mut [u32], &mut [u32]) = (&mut [3, 4], &mut [5]);
    ///
    /// assert_eq!(array_queue.as_mut_slices(), should_be);
    /// ```
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        let phys_head = self.to_physical_idx_from_head(0);
        let phys_tail = self.to_physical_idx_from_head(self.len());

        if phys_tail > phys_head {
            (
                unsafe { &mut *slice_from_raw_parts_mut(self.array.as_mut_ptr().add(phys_head).cast(), self.len) },
                &mut [],
            )
        } else {
            (
                unsafe { &mut *slice_from_raw_parts_mut(self.array.as_mut_ptr().add(phys_head).cast(), N - phys_head) },
                unsafe { &mut *slice_from_raw_parts_mut(self.array.as_mut_ptr().cast(), phys_tail) },
            )
        }
    }

    /// Increases the head index by the specified number and decreases the length by the same number.
    ///
    /// # Safety
    ///
    /// The caller must ensure usage of items that become available after this function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut queue = ArrayQueue::from([1, 2, 3, 4]);
    ///
    /// queue.pop().unwrap();
    /// queue.push(5).unwrap();
    ///
    /// let slices = queue.as_mut_slices();
    /// let should_be: (&mut [u32], &mut [u32]) = (&mut [2, 3, 4], &mut [5]);
    /// assert_eq!(slices, should_be);
    ///
    /// for item in slices.0.iter_mut() {
    ///     // Do something with items
    ///     unsafe { std::ptr::drop_in_place(item); } // Ensure the items are dropped
    /// }
    ///
    /// // Here head is 1 and len is 4
    ///
    /// let slices = queue.as_slices();
    /// let as_previous: (&[u32], &[u32]) = (&[2, 3, 4], &[5]); // But the queue is still the same, while 3 elements were read
    /// assert_eq!(slices, as_previous);
    ///
    /// unsafe { queue.inc_head_by(3); }
    ///
    /// // Here head is 0 (4 is wrapped around), and len is 1
    ///
    /// let slices = queue.as_slices();
    /// let should_be: (&[u32], &[u32]) = (&[5], &[]);
    /// assert_eq!(slices, should_be); // Now it is valid
    /// ```
    pub unsafe fn inc_head_by(&mut self, number: usize) {
        self.head = self.to_physical_idx_from_head(number);
        self.len -= number;
    }

    /// Decreases the length by the specified number.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the length is not less than the specified number.
    /// And the caller must ensure usage of items that become available after this function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut queue = ArrayQueue::from([1, 2, 3, 4]);
    ///
    /// queue.pop().unwrap();
    /// queue.pop().unwrap();
    /// queue.push(5).unwrap();
    /// queue.push(6).unwrap();
    ///
    /// let slices = queue.as_mut_slices();
    /// let should_be: (&mut [u32], &mut [u32]) = (&mut [3, 4], &mut [5, 6]);
    /// assert_eq!(slices, should_be);
    ///
    /// for item in slices.1.iter_mut() {
    ///     // Do something with items
    ///     unsafe { std::ptr::drop_in_place(item); } // Ensure the items are dropped
    /// }
    ///
    /// // Here head is 2 and len is 4
    ///
    /// let slices = queue.as_slices();
    /// let as_previous: (&[u32], &[u32]) = (&[3, 4], &[5, 6]); // But the queue is still the same, while 2 elements were read
    /// assert_eq!(slices, as_previous);
    ///
    /// unsafe { queue.dec_len_by(2); }
    ///
    /// // Here head is 2 and len is 2
    ///
    /// let slices = queue.as_slices();
    /// let should_be: (&[u32], &[u32]) = (&[3, 4], &[]);
    /// assert_eq!(slices, should_be); // Now it is valid
    /// ```
    pub unsafe fn dec_len_by(&mut self, number: usize) {
        debug_assert!(self.len >= number);

        self.len -= number;
    }

    /// Appends an element to the back of the queue.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the queue is not full.
    pub unsafe fn push_unchecked(&mut self, value: T) {
        assert_hint(self.len() < N, "Tried to push to a full array stack");

        let idx = self.to_physical_idx_from_head(self.len());

        unsafe { ptr::write(self.array.get_unchecked_mut(idx), MaybeUninit::new(value)) };

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

    /// Pushes the provided value to the front of the queue.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the queue is not full.
    pub unsafe fn push_priority_value_unchecked(&mut self, value: T) {
        assert_hint(self.len() < N, "Tried to push to a full array stack");

        let phys_head = self.to_physical_idx_from_head(0);
        let idx = if likely(phys_head > 0) {
            phys_head - 1
        } else {
            N - 1
        };

        unsafe { ptr::write(self.array.get_unchecked_mut(idx), MaybeUninit::new(value)) };

        self.head = idx;
        self.len += 1;
    }

    /// Pushes the provided value to the front of the queue
    /// or returns `Err(value)` if the queue is full.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut queue = ArrayQueue::<u32, 3>::new();
    ///
    /// queue.push_priority_value(1).unwrap(); // [1, _, _]
    /// queue.push(2).unwrap(); // [1, 2, _]
    /// queue.push_priority_value(3).unwrap(); // [3, 1, 2]
    ///
    /// assert_eq!(queue.pop(), Some(3));
    /// assert_eq!(queue.pop(), Some(1));
    /// assert_eq!(queue.pop(), Some(2));
    /// ```
    pub fn push_priority_value(&mut self, value: T) -> Result<(), T> {
        if likely(self.len() < N) {
            unsafe { self.push_priority_value_unchecked(value) };

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
            self.head = self.to_physical_idx_from_head(1);

            assert_hint(
                self.array.len() > idx,
                &format!("idx: {}, len: {}", idx, self.array.len()),
            );

            Some(unsafe { self.array.get_unchecked_mut(idx).assume_init_read() })
        } else {
            None
        }
    }

    /// Removes the last element and returns it, or `None` if the queue is empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::ArrayQueue;
    ///
    /// let mut queue = ArrayQueue::<u32, 3>::new();
    ///
    /// queue.push(1).unwrap(); // [1, _, _]
    /// queue.push(2).unwrap(); // [1, 2, _]
    /// queue.push(3).unwrap(); // [1, 2, 3]
    ///
    /// assert_eq!(queue.pop_less_priority_value(), Some(3));
    /// assert_eq!(queue.pop(), Some(1));
    /// assert_eq!(queue.pop(), Some(2));
    /// ```
    pub fn pop_less_priority_value(&mut self) -> Option<T> {
        if !self.is_empty() {
            self.len -= 1;

            let idx = self.to_physical_idx_from_head(self.len());

            Some(unsafe { self.array.get_unchecked_mut(idx).assume_init_read() })
        } else {
            None
        }
    }

    /// Clears with calling the provided function on each element.
    pub fn clear_with<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        for i in 0..self.len {
            let idx = self.to_physical_idx_from_head(i);

            let value = unsafe { self.array.get_unchecked_mut(idx).assume_init_read() };

            f(value);
        }

        self.len = 0;
    }

    /// Drops all elements in the queue and set the length to 0.
    pub fn clear(&mut self) {
        if mem::needs_drop::<T>() {
            for i in 0..self.len {
                let idx = self.to_physical_idx_from_head(i);

                unsafe { ptr::drop_in_place(self.array.get_unchecked_mut(idx)) };
            }
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
                    let idx = self.queue.to_physical_idx_from_head(self.iterated);

                    self.iterated += 1;

                    Some(unsafe { self.queue.array.get_unchecked(idx).assume_init_ref() })
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
                    let idx = self.queue.to_physical_idx_from_head(self.iterated);

                    self.iterated += 1;

                    Some(unsafe { &mut *(self.queue.array.get_unchecked_mut(idx) as *mut _ as *mut T) })
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
    pub unsafe fn refill_with(&mut self, f: impl FnOnce(&mut [MaybeUninit<T>; N]) -> usize) {
        debug_assert!(self.is_empty(), "ArrayQueue should be empty before refilling");

        let filled = f(&mut self.array);

        debug_assert!(filled <= N, "Filled more than the capacity");

        self.len = filled;
        self.head = 0;
    }

    /// Returns a pointer to the array as if an array of `T`.
    fn as_slice_ptr(&self) -> *const [T; N] {
        (&raw const self.array).cast()
    }
}

impl<T, const N: usize> Deref for ArrayQueue<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.as_slice_ptr() }
    }
}

impl<T, const N: usize> AsRef<[T]> for ArrayQueue<T, N> {
    fn as_ref(&self) -> &[T] {
        unsafe { &*self.as_slice_ptr() }
    }
}

impl<T, const N: usize> DerefMut for ArrayQueue<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.as_slice_ptr().cast_mut() }
    }
}

impl<T, const N: usize> AsMut<[T]> for ArrayQueue<T, N> {
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { &mut *self.as_slice_ptr().cast_mut() }
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
            array: unsafe { (&raw const array).cast::<[MaybeUninit<T>; N]>().read() },
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
                array.copy_from_slice(&[
                    MaybeUninit::new(1),
                    MaybeUninit::new(2),
                    MaybeUninit::new(3),
                    MaybeUninit::new(4)
                ]);

                4
            });
        };

        assert_eq!(queue.len(), 4);
        assert_eq!(queue.iter().collect::<Vec<_>>(), vec![&1, &2, &3, &4]);
    }
}