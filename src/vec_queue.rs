//! This module provides the [`VecQueue`] an vector-based queue implementation.

use std::{mem, ptr};
use std::ptr::slice_from_raw_parts;
use crate::hints::unlikely;

/// A queue that uses a vector to store the elements.
///
/// Unlike [`std::collections::VecDeque`], this queue is not double-ended.
pub struct VecQueue<T> {
    ptr: *mut T,
    head: usize,
    tail: usize,
    capacity: usize,
    mask: usize,
}

impl<T> VecQueue<T> {
    /// Allocates a new vector with the given capacity.
    #[cold]
    fn allocate(capacity: usize) -> *mut T {
        debug_assert!(capacity > 0 && capacity.is_power_of_two());

        unsafe {
            std::alloc::alloc(std::alloc::Layout::array::<T>(capacity).unwrap_unchecked()).cast()
        }
    }

    /// Deallocates a vector with the given capacity.
    #[cold]
    fn deallocate(ptr: *mut T, capacity: usize) {
        unsafe {
            std::alloc::dealloc(
                ptr.cast(),
                std::alloc::Layout::array::<T>(capacity).unwrap_unchecked(),
            );
        }
    }

    /// Returns the mask for the given capacity.
    const fn get_mask_for_capacity(capacity: usize) -> usize {
        debug_assert!(capacity.is_power_of_two());

        capacity - 1
    }

    /// Returns the physical index for the given index.
    #[inline(always)]
    fn get_physical_index(&self, index: usize) -> usize {
        debug_assert!(self.capacity.is_power_of_two());

        index & self.mask
    }

    /// Creates a new `VecQueue` without any capacity.
    pub const fn new_const() -> Self {
        Self {
            ptr: ptr::null_mut(),
            head: 0,
            tail: 0,
            capacity: 0,
            mask: 0,
        }
    }

    /// Creates a new `VecQueue` with the default capacity.
    pub fn new() -> Self {
        const DEFAULT_CAPACITY: usize = 16;

        Self {
            ptr: Self::allocate(DEFAULT_CAPACITY),
            head: 0,
            tail: 0,
            capacity: DEFAULT_CAPACITY,
            mask: Self::get_mask_for_capacity(DEFAULT_CAPACITY),
        }
    }

    /// Returns the number of elements in the queue.
    pub fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// Extends the vector to the given capacity.
    ///
    /// # Panics
    ///
    /// Panics if the provided capacity is not a power of two or is less than the current capacity.
    #[inline(never)]
    #[cold]
    #[track_caller]
    pub fn extend_to(&mut self, capacity: usize) {
        #[inline(never)]
        #[cold]
        fn extend_from_zero<T>(queue: &mut VecQueue<T>, capacity: usize) {
            queue.mask = VecQueue::<T>::get_mask_for_capacity(capacity);
            queue.ptr = VecQueue::<T>::allocate(capacity);
            queue.capacity = capacity;
        }

        if unlikely(self.capacity == 0 && capacity == 0) {
            extend_from_zero(self, 4);

            return;
        }

        assert!(capacity.is_power_of_two(), "Capacity must be a power of two, provided {capacity}");
        assert!(capacity > self.capacity);

        let new_ptr = Self::allocate(capacity);
        let len = self.len();

        unsafe {
            let phys_head = self.get_physical_index(self.head);
            let phys_tail = self.get_physical_index(self.tail);
            let src = self.ptr.add(phys_head);
            let dst = new_ptr;

            if phys_head < phys_tail {
                ptr::copy(src, dst, len);
            } else {
                ptr::copy(src, dst, self.capacity - phys_head);

                let src = self.ptr;
                let dst = new_ptr.add(self.capacity - phys_head);

                ptr::copy(src, dst, phys_tail);
            }
        }

        Self::deallocate(self.ptr, self.capacity);

        self.head = 0;
        self.tail = len;
        self.ptr = new_ptr;
        self.capacity = capacity;
        self.mask = Self::get_mask_for_capacity(capacity);
    }

    /// Pushes a value to the queue.
    #[inline]
    pub fn push(&mut self, value: T) {
        if unlikely(self.len() == self.capacity) {
            self.extend_to(self.capacity * 2);
        }

        unsafe {
            let index = self.get_physical_index(self.tail);

            self.ptr.add(index).write(value);
        }

        self.tail = self.tail.wrapping_add(1);
    }

    /// Pops a value from the queue.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let index = self.get_physical_index(self.head);
        let value = unsafe { self.ptr.add(index).read() };

        self.head = self.head.wrapping_add(1);

        Some(value)
    }

    /// Pushes a slice to the queue.
    ///
    /// # Safety
    ///
    /// It `T` is not `Copy`, the caller should [`forget`](mem::forget) the values.
    #[inline]
    pub unsafe fn extend_from_slice(&mut self, slice: &[T]) {
        let needed = self.len() + slice.len();

        if unlikely(needed > self.capacity) {
            let mut new_capacity = self.capacity;
            while new_capacity < needed {
                new_capacity *= 2;
            }

            self.extend_to(new_capacity);
        }

        let phys_tail = self.get_physical_index(self.tail);
        let right_space = self.capacity - phys_tail;

        unsafe {
            if slice.len() <= right_space {
                // fits in one memcpy
                ptr::copy_nonoverlapping(slice.as_ptr(), self.ptr.add(phys_tail), slice.len());
            } else {
                // wraparound case
                ptr::copy_nonoverlapping(slice.as_ptr(), self.ptr.add(phys_tail), right_space);

                ptr::copy_nonoverlapping(
                    slice.as_ptr().add(right_space),
                    self.ptr,
                    slice.len() - right_space,
                );
            }
        }

        self.tail = self.tail.wrapping_add(slice.len());
    }


    /// Accepts a function that will be called with the slices of the queue to move.
    ///
    /// # Safety
    ///
    /// The function should copy all elements from the provided slices.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::VecQueue;
    ///
    /// let mut queue = VecQueue::new();
    ///
    /// for i in 0..10 {
    ///     queue.push(i);
    /// }
    ///
    /// let mut receiver = Vec::with_capacity(10);
    ///
    /// unsafe {
    ///     queue.take_batch(|first_slice, second_slice| {
    ///         receiver.extend_from_slice(first_slice);
    ///         receiver.extend_from_slice(second_slice);
    ///     }, 8);
    /// }
    ///
    /// assert_eq!(receiver, (0..8).collect::<Vec<_>>());
    /// assert_eq!(queue.len(), 2);
    /// assert_eq!(queue.pop(), Some(8));
    /// assert_eq!(queue.pop(), Some(9));
    /// ```
    pub unsafe fn take_batch<F: FnOnce(&[T], &[T])>(&mut self, f: F, mut limit: usize) {
        limit = self.len().min(limit);

        let phys_head = self.get_physical_index(self.head);
        let right_occupied = self.capacity - phys_head;

        self.head = self.head.wrapping_add(limit);

        if limit <= right_occupied {
            // We can copy from the head to the head + limit.
            f(
                unsafe {&*slice_from_raw_parts(self.ptr.add(phys_head), limit) },
                &[],
            );

            // The head is already updated.

            return;
        }

        let slice1 = unsafe { &*slice_from_raw_parts(self.ptr.add(phys_head), right_occupied) };
        let slice2 = unsafe { &*slice_from_raw_parts(self.ptr, limit - right_occupied) };

        f(slice1, slice2);

        // The head is already updated.
    }

    /// Clears the queue by calling the provided function on each element.
    pub fn clear_with<F: Fn(T)>(&mut self, f: F) {
        for i in 0..self.len() {
            let elem = unsafe { self.ptr.add(self.get_physical_index(self.head + i)).read() };

            f(elem);
        }

        self.head = 0;
        self.tail = 0;
    }

    /// Clears the queue.
    pub fn clear(&mut self) {
        self.clear_with(drop);
    }

    /// Returns an iterator over the queue.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        struct Iter<'queue, T> {
            queue: &'queue VecQueue<T>,
            current_head: usize,
        }

        impl<'queue, T> Iterator for Iter<'queue, T> {
            type Item = &'queue T;

            fn next(&mut self) -> Option<Self::Item> {
                if unlikely(self.current_head == self.queue.tail) {
                    return None;
                }

                let index = self.queue.get_physical_index(self.current_head);

                self.current_head = self.current_head.wrapping_add(1);

                Some(unsafe { &*self.queue.ptr.add(index) })
            }
        }

        Iter {
            queue: self,
            current_head: self.head,
        }
    }

    /// Returns a mutable iterator over the queue.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        struct Iter<'queue, T> {
            queue: &'queue mut VecQueue<T>,
            current_head: usize,
        }

        impl<'queue, T> Iterator for Iter<'queue, T> {
            type Item = &'queue mut T;

            fn next(&mut self) -> Option<Self::Item> {
                if unlikely(self.current_head == self.queue.tail) {
                    return None;
                }

                let index = self.queue.get_physical_index(self.current_head);

                self.current_head = self.current_head.wrapping_add(1);

                Some(unsafe { &mut *self.queue.ptr.add(index) })
            }
        }

        let head = self.head;

        Iter {
            queue: self,
            current_head: head,
        }
    }
}

impl<T: Clone> Clone for VecQueue<T> {
    fn clone(&self) -> Self {
        let mut new = Self::new();

        new.extend_to(new.capacity);

        for i in 0..self.len() {
            let elem = unsafe { &*self.ptr.add(self.get_physical_index(self.head + i)) };

            new.push(elem.clone());
        }

        new
    }
}

impl<T> Default for VecQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for VecQueue<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            while let Some(val) = self.pop() {
                drop(val);
            }
        }

        Self::deallocate(self.ptr, self.capacity);
    }
}