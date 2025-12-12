//! This module provides the [`NumberKeyMap`] struct.
//!
//! The [`NumberKeyMap`] is a compact, open-addressing map specialized for `usize` keys
//! and generic values `V`.
//! The implementation stores an internal contiguous array of `Slot<V>` entries and performs
//! direct indexing of slots based on `key % capacity` with a small number of probe steps.
//!
//! The [`NumberKeyMap`] is optimized for zero-misses and so optimized for 99+% reading operations.

use crate::hints::{assert_hint, cold_path, likely, unlikely, unwrap_or_bug_hint};
use alloc::alloc::{alloc, dealloc, Layout};
use core::ptr::null_mut;
use core::{mem, ptr};

/// Internal error codes used by the low-level insertion routine.
///
/// This enum describes why `insert_or_fail` failed: either there was no free slot
/// available inside the currently probed region, or the same key was already present.
enum InsertFailErr {
    /// Not enough free slots were found in the target probing region.
    NotEnoughSpace,
    /// The key is already present in the map.
    KeyAlreadyExists,
}

/// A single map slot that stores a key together with its associated value.
///
/// `Slot` is the in-memory element type of the internal contiguous buffer. Keys that are
/// unused are expected to equal `usize::MAX`.
struct Slot<V> {
    key: usize,
    value: V,
}

/// A small, specialized hash map keyed by `usize` values.
///
/// It is optimized for zero-misses and so optimized for 99+% reading operations.
///
/// It is optimized for integer keys and expects an external invariant
/// that `usize::MAX` marks vacant slots.
///
/// # Example
///
/// ```rust
/// use std::sync::{Mutex, RwLock};
/// use orengine_utils::NumberKeyMap;
///
/// static POOLS: RwLock<NumberKeyMap<Mutex<Vec<Box<[u8]>>>>> = RwLock::new(NumberKeyMap::new());
///
/// fn acquire_from_pool(size: usize) -> Box<[u8]> {
///     if let Some(v) = POOLS.read().unwrap().get(size) {
///         v.lock().unwrap().pop().unwrap_or_else(|| vec![0; size].into_boxed_slice())
///     } else {
///         vec![0; size].into_boxed_slice()
///     }
/// }
///
/// fn put_to_pool(buf: Box<[u8]>) {
///     let read_pool = POOLS.read().unwrap();
///
///     if let Some(v) = read_pool.get(buf.len()) {
///         v.lock().unwrap().push(buf);
///         return;
///     }
///
///     drop(read_pool);
///
///     let mut write_pool = POOLS.write().unwrap();
///     let res = write_pool.insert(buf.len(), Mutex::new(vec![buf]));
///
///     if let Err(mut v) = res {
///         let buf = v.get_mut().unwrap().pop().unwrap();
///
///         write_pool.get(buf.len()).unwrap().lock().unwrap().push(buf);
///     }
/// }
/// ```
pub struct NumberKeyMap<V> {
    // Because max misses are 0 for now and should be very close to it in the future,
    // we should use `*mut Slot<V>` instead of *mut [key] and *mut [value].
    inner: *mut Slot<V>,
    capacity: usize,
}

impl<V> NumberKeyMap<V> {
    /// Create an empty `NumberKeyMap`.
    pub const fn new() -> Self {
        Self {
            inner: null_mut(),
            capacity: 0,
        }
    }

    /// Compute the start index in the buffer for the provided `key` and `capacity`.
    ///
    /// This is the primary hash function used by the map: `key % capacity`.
    fn get_started_slot_idx_for_key(key: usize, capacity: usize) -> usize {
        key % capacity
    }

    /// Validate a key for use in the map.
    ///
    /// The implementation reserves `usize::MAX` as the special vacant key marker, so
    /// real keys must not be equal to that value.
    fn validate_key(key: usize) {
        assert_hint(key != usize::MAX, "`key` should be not equal to usize::MAX");
    }

    /// Compute a raw pointer to the slot at `idx` inside the `inner` buffer.
    ///
    /// # Panics
    ///
    /// This function uses `assert_hint` to check that `inner` is not null and that
    /// `idx < capacity`. These checks are considered programmer errors in the
    /// original code and will abort the program if violated.
    fn get_slot_ptr(inner: *mut Slot<V>, capacity: usize, idx: usize) -> *mut Slot<V> {
        assert_hint(
            !inner.is_null(),
            "NumberKeyMap is allocated at `get_slot_ptr`",
        );
        assert_hint(idx < capacity, "`idx` is out of bounds at `get_slot_ptr`");

        unsafe { inner.add(idx) }
    }

    /// Get an immutable reference to the slot at `idx`.
    fn get_slot(&self, idx: usize) -> &Slot<V> {
        unsafe { &*Self::get_slot_ptr(self.inner, self.capacity, idx) }
    }

    /// Get a mutable reference to the slot at `idx`.
    fn get_slot_mut(&mut self, idx: usize) -> &mut Slot<V> {
        unsafe { &mut *Self::get_slot_ptr(self.inner, self.capacity, idx) }
    }

    /// Retrieve a reference to a value stored under `key`, if present.
    ///
    /// If the slot is occupied and contains the requested key, a reference to the
    /// value is returned.
    ///
    /// # Panics
    ///
    /// This function panics if `key` is equal to `usize::MAX`.
    pub fn get(&self, key: usize) -> Option<&V> {
        Self::validate_key(key);

        if unlikely(self.inner.is_null()) {
            return None;
        }

        let idx = Self::get_started_slot_idx_for_key(key, self.capacity);
        let slot = self.get_slot(idx);

        if likely(slot.key == key) {
            return Some(&slot.value);
        }

        None
    }

    /// Retrieve a mutable reference to a value stored under `key`, if present.
    ///
    /// If the slot is occupied and contains the requested key, a reference to the
    /// value is returned.
    ///
    /// # Panics
    ///
    /// This function panics if `key` is equal to `usize::MAX`.
    pub fn get_mut(&mut self, key: usize) -> Option<&mut V> {
        Self::validate_key(key);

        if unlikely(self.inner.is_null()) {
            return None;
        }

        let idx = Self::get_started_slot_idx_for_key(key, self.capacity);
        let slot = self.get_slot_mut(idx);

        if likely(slot.key == key) {
            return Some(&mut slot.value);
        }

        None
    }

    /// Compute a larger capacity when resizing is needed.
    ///
    /// The growth formula is conservative for small capacities and uses a factor
    /// of `8/7` for larger ones. The function ensures the new capacity is never
    /// a power of two (it increments by one if so) to prevent bad key distributing.
    fn greater_capacity(capacity: usize) -> usize {
        if unlikely(capacity < 16) {
            return capacity * 2 + 2; // 1 -> 4 -> 10 -> 22 and next this condition is always false
        }

        let new_capacity = capacity * 8 / 7;
        if unlikely(new_capacity.is_power_of_two()) {
            new_capacity + 1
        } else {
            new_capacity
        }
    }

    /// Low-level attempt to insert a value into an already-allocated buffer.
    ///
    /// Tries to write `value_ptr.read()` into the slot chosen by `key % capacity`.
    /// On success returns `Ok(())`. On failure returns `Err(InsertFailErr)` with the
    /// reason: either `NotEnoughSpace` when the slot is not vacant, or `KeyAlreadyExists`
    /// if the same key is found and the caller semantics expect that.
    ///
    /// # Safety
    ///
    /// On success the caller must forget the `value_ptr`.
    unsafe fn insert_or_fail(
        inner: *mut Slot<V>,
        capacity: usize,
        key: usize,
        value_ptr: *const V,
    ) -> Result<(), InsertFailErr> {
        assert_hint(
            !inner.is_null(),
            "null pointer is provided to `insert_or_fail`",
        );

        let idx = Self::get_started_slot_idx_for_key(key, capacity);
        let slot_ptr = Self::get_slot_ptr(inner, capacity, idx);
        let slot = unsafe { &mut *slot_ptr };

        if likely(slot.key == usize::MAX) {
            unsafe {
                slot_ptr.write(Slot {
                    key,
                    value: value_ptr.read(),
                });
            }

            Ok(())
        } else if unlikely(key == slot.key) {
            Err(InsertFailErr::KeyAlreadyExists)
        } else {
            // slot.key != usize::MAX && slot.key != key = occupied by another key
            Err(InsertFailErr::NotEnoughSpace)
        }
    }

    /// Increases the capacity of the map and inserts `key`/`value` into the new buffer.
    ///
    /// This method is marked `#[cold]` and `#[inline(never)]` because it is expected
    /// to run rarely (only on reallocation). It allocates a larger buffer, attempts to
    /// copy existing entries into it, and finally inserts the provided `(key, value)`.
    ///
    /// # Errors
    ///
    /// Returns `Err(V)` only when the `key` already exists in the map; otherwise it
    /// commits the reallocation and returns `Ok(())`.
    #[cold]
    #[inline(never)]
    fn slow_insert(&mut self, key: usize, value: V) -> Result<(), V> {
        let mut new_capacity = Self::greater_capacity(self.capacity);

        'allocate: loop {
            let layout = unwrap_or_bug_hint(Layout::array::<Slot<V>>(new_capacity));
            // It is more expensive to first check if the capacity is good enough
            // for zero-misses and only after allocate and insert
            // than inserts from the start and reallocate if needed.
            let new_inner: *mut Slot<V> = unsafe { alloc(layout) }.cast();

            for i in 0..new_capacity {
                unsafe {
                    let slot = new_inner.add(i);

                    (*slot).key = usize::MAX;
                };
            }

            for idx in 0..self.capacity {
                let slot = self.get_slot(idx);

                if slot.key != usize::MAX {
                    let res = unsafe {
                        Self::insert_or_fail(new_inner, new_capacity, slot.key, &slot.value)
                    };
                    if unlikely(res.is_err()) {
                        assert_hint(
                            matches!(res, Err(InsertFailErr::NotEnoughSpace)),
                            "invalid inner state is detected while reallocating: duplicate key",
                        );

                        // We should reallocate
                        new_capacity = Self::greater_capacity(new_capacity);

                        unsafe { dealloc(new_inner.cast(), layout) };

                        continue 'allocate;
                    }
                }
            }

            // We recopied all the values, but we need to insert one more item.
            let res = unsafe { Self::insert_or_fail(new_inner, new_capacity, key, &value) };

            let mut commit_reallocate = || {
                unsafe {
                    dealloc(
                        self.inner.cast(),
                        unwrap_or_bug_hint(Layout::array::<Slot<V>>(self.capacity)),
                    );
                };

                self.inner = new_inner;
                self.capacity = new_capacity;
            };

            match res {
                Ok(()) => {
                    commit_reallocate();

                    mem::forget(value);

                    break Ok(());
                }

                Err(InsertFailErr::NotEnoughSpace) => {
                    cold_path();

                    // We should reallocate
                    new_capacity = Self::greater_capacity(new_capacity);

                    unsafe { dealloc(new_inner.cast(), layout) };

                    continue 'allocate;
                }

                Err(InsertFailErr::KeyAlreadyExists) => {
                    // We have already successfully resized the map, and one next insert will need it,
                    // so we commit the resize but return an error

                    commit_reallocate();

                    break Err(value);
                }
            }
        }
    }

    /// Allocates the map with one `key`/`value`.
    #[cold]
    #[inline(never)]
    fn insert_first(&mut self, key: usize, value: V) {
        Self::validate_key(key);

        let layout = unwrap_or_bug_hint(Layout::array::<Slot<V>>(1));
        let inner: *mut Slot<V> = unsafe { alloc(layout) }.cast();
        unsafe { inner.write(Slot { key, value }) };

        self.inner = inner;
        self.capacity = 1;
    }

    /// Insert a key/value pair into the map.
    ///
    /// # Note
    ///
    /// This operation is very expensive! If you want to call it frequently,
    /// consider using a `HashMap` instead.
    ///
    /// # Errors
    ///
    /// Returns `Err(V)` if the key already exists in the map; otherwise returns `Ok(())`.
    ///
    /// # Panics
    ///
    /// This function panics if `key` is equal to `usize::MAX`
    pub fn insert(&mut self, key: usize, value: V) -> Result<(), V> {
        Self::validate_key(key);

        if unlikely(self.inner.is_null()) {
            self.insert_first(key, value);

            return Ok(());
        }

        let res = unsafe { Self::insert_or_fail(self.inner, self.capacity, key, &value) };
        if likely(res.is_ok()) {
            mem::forget(value);

            return Ok(());
        }

        self.slow_insert(key, value)
    }

    /// Removes an item from the map and returns it if it exists.
    ///
    /// # Panics
    ///
    /// This function panics if `key` is equal to `usize::MAX`
    pub fn remove(&mut self, key: usize) -> Option<V> {
        Self::validate_key(key);

        let idx = Self::get_started_slot_idx_for_key(key, self.capacity);
        let slot = self.get_slot_mut(idx);
        if unlikely(slot.key == usize::MAX) {
            return None;
        }

        slot.key = usize::MAX;

        Some(unsafe { ptr::read(&slot.value) })
    }

    /// Clears the [`NumberKeyMap`] with the provided function.
    pub fn clear_with(&mut self, func: impl Fn((usize, V))) {
        if self.inner.is_null() {
            return;
        }

        for i in 0..self.capacity {
            let slot_ptr = unsafe { self.inner.add(i) };
            let slot = unsafe { &mut *slot_ptr };

            if slot.key != usize::MAX {
                func((slot.key, unsafe { ptr::read(&slot.value) }));
                slot.key = usize::MAX;
            }
        }
    }

    /// Clears the [`NumberKeyMap`].
    pub fn clear(&mut self) {
        self.clear_with(drop);
    }
}

impl<V> Default for NumberKeyMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// An iterator over the [`NumberKeyMap`].
/// The item of this iterator is `(key, value)`.
///
/// This iterator consumes the [`NumberKeyMap`].
pub struct IntoIter<V> {
    start: *mut Slot<V>,
    i: usize,
    capacity: usize,
}

impl<V> Iterator for IntoIter<V> {
    type Item = (usize, V);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            while self.i < self.capacity {
                let ptr = self.start.add(self.i);
                let slot = &mut *ptr;

                self.i += 1;

                if slot.key != usize::MAX {
                    return Some((slot.key, ptr::read(&slot.value)));
                }
            }

            None
        }
    }
}

impl<V> Drop for IntoIter<V> {
    fn drop(&mut self) {
        unsafe {
            // Drop remaining values
            for (_k, v) in self.by_ref() {
                drop(v);
            }

            // Free memory
            let layout = Layout::array::<Slot<V>>(self.capacity).unwrap();

            dealloc(self.start.cast(), layout);
        }
    }
}

impl<V> NumberKeyMap<V> {
    /// Iterate immutably over all `(key, &value)`.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &V)> {
        struct Iter<'a, V> {
            ptr: *mut Slot<V>,
            end: *mut Slot<V>,
            _marker: core::marker::PhantomData<&'a V>,
        }

        impl<'a, V> Iterator for Iter<'a, V> {
            type Item = (usize, &'a V);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    while self.ptr < self.end {
                        let slot = &*self.ptr;

                        self.ptr = self.ptr.add(1);

                        if slot.key != usize::MAX {
                            return Some((slot.key, &slot.value));
                        }
                    }

                    None
                }
            }
        }

        Iter {
            ptr: self.inner,
            end: unsafe { self.inner.add(self.capacity) },
            _marker: core::marker::PhantomData,
        }
    }

    /// Iterate mutably over all `(key, &mut value)`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut V)> {
        struct IterMut<'a, V> {
            ptr: *mut Slot<V>,
            end: *mut Slot<V>,
            _marker: core::marker::PhantomData<&'a mut V>,
        }

        impl<'a, V: 'a> Iterator for IterMut<'a, V> {
            type Item = (usize, &'a mut V);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    while self.ptr < self.end {
                        let slot = &mut *self.ptr;

                        self.ptr = self.ptr.add(1);

                        if slot.key != usize::MAX {
                            return Some((slot.key, &mut slot.value));
                        }
                    }

                    None
                }
            }
        }

        IterMut {
            ptr: self.inner,
            end: unsafe { self.inner.add(self.capacity) },
            _marker: core::marker::PhantomData,
        }
    }
}

impl<V: 'static> NumberKeyMap<V> {
    /// Remove all entries and yield owned `(key, value)`.
    pub fn drain(&mut self) -> impl Iterator<Item = (usize, V)> {
        struct Drain<'a, V> {
            ptr: *mut Slot<V>,
            end: *mut Slot<V>,
            _marker: core::marker::PhantomData<&'a mut V>,
        }

        impl<V: 'static> Iterator for Drain<'_, V> {
            type Item = (usize, V);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    while self.ptr < self.end {
                        let slot = &mut *self.ptr;

                        self.ptr = self.ptr.add(1);

                        if slot.key != usize::MAX {
                            let key = slot.key;

                            slot.key = usize::MAX;

                            return Some((key, ptr::read(&slot.value)));
                        }
                    }

                    None
                }
            }
        }

        Drain {
            ptr: self.inner,
            end: unsafe { self.inner.add(self.capacity) },
            _marker: core::marker::PhantomData,
        }
    }
}

impl<V> IntoIterator for NumberKeyMap<V> {
    type Item = (usize, V);
    type IntoIter = IntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        let iter = IntoIter {
            start: self.inner,
            i: 0,
            capacity: self.capacity,
        };

        mem::forget(self);

        iter
    }
}

unsafe impl<V> Sync for NumberKeyMap<V> {}
unsafe impl<V> Send for NumberKeyMap<V> {}

impl<V> Drop for NumberKeyMap<V> {
    fn drop(&mut self) {
        if self.inner.is_null() {
            return;
        }

        if mem::needs_drop::<V>() {
            for i in 0..self.capacity {
                let slot_ptr = unsafe { self.inner.add(i) };
                let slot = unsafe { &mut *slot_ptr };

                if slot.key != usize::MAX {
                    unsafe { (&raw mut slot.value).drop_in_place() };
                }
            }
        }

        let layout = Layout::array::<Slot<V>>(self.capacity).unwrap();
        unsafe {
            dealloc(self.inner.cast(), layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::rc::Rc;
    #[cfg(feature = "no_std")]
    use alloc::vec::Vec;
    use core::cell::Cell;

    #[derive(Debug)]
    struct DropCounter(usize, Rc<Cell<usize>>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }

    #[test]
    fn test_number_key_map_insert_and_get() {
        const N: usize = 1_000_000;

        let mut m = NumberKeyMap::new();
        let drops = Rc::new(Cell::new(0));

        for i in 0..N {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();

            assert_eq!(m.get(i).map(|v| v.0), Some(i));
            assert_eq!(m.get_mut(i).map(|v| v.0), Some(i));
        }

        for i in 0..N {
            assert_eq!(m.get(i).map(|v| v.0), Some(i));
        }

        assert_eq!(drops.get(), 0);

        for i in 0..N / 2 {
            assert!(m.remove(i).is_some());
            assert!(m.remove(i).is_none());
        }

        assert_eq!(drops.get(), N / 2);

        drop(m);

        assert_eq!(drops.get(), N);
    }

    #[test]
    fn test_number_key_map_duplicate_key_returns_err() {
        let mut m = NumberKeyMap::new();
        let k = 1usize;
        let drops = Rc::new(Cell::new(0));

        m.insert(k, DropCounter(10, drops.clone())).unwrap();
        assert!(m.insert(k, DropCounter(20, drops.clone())).is_err());

        // original value remains
        assert_eq!(m.get(k).map(|v| v.0), Some(10));

        assert_eq!(drops.get(), 1);

        drop(m);

        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn test_number_key_map_clear() {
        let mut m = NumberKeyMap::new();
        let drops = Rc::new(Cell::new(0));

        for i in 0..1_000_000 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        assert_eq!(drops.get(), 0);

        m.clear();

        assert_eq!(drops.get(), 1_000_000);

        m.clear_with(|_| panic!("Not cleared"));

        assert_eq!(drops.get(), 1_000_000);
    }

    #[test]
    fn test_number_key_map_iter() {
        let mut m = NumberKeyMap::new();
        let drops = Rc::new(Cell::new(0));

        for i in 0..10 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        let mut seen = Vec::new();
        for (k, v) in m.iter() {
            seen.push((k, v.0));
        }

        seen.sort_by_key(|x| x.0);

        assert_eq!(seen, (0..10).map(|i| (i, i)).collect::<Vec<_>>());
        assert_eq!(drops.get(), 0); // iter() should not drop
    }

    #[test]
    fn test_number_key_map_iter_mut() {
        let mut m = NumberKeyMap::new();
        let drops = Rc::new(Cell::new(0));

        for i in 0..10 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        for (_, v) in m.iter_mut() {
            v.0 *= 2;
        }

        let mut collected = m.iter().map(|(_, v)| v.0).collect::<Vec<_>>();

        collected.sort_by_key(|x| *x);

        assert_eq!(collected, (0..10).map(|i| i * 2).collect::<Vec<_>>());
        assert_eq!(drops.get(), 0); // iter_mut() should not drop
    }

    #[test]
    fn test_number_key_map_into_iter() {
        let drops = Rc::new(Cell::new(0));
        let mut m = NumberKeyMap::new();

        for i in 0..10 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        assert_eq!(drops.get(), 0);

        let mut seen = Vec::new();
        for (k, v) in m {
            seen.push((k, v.0));
        }

        // after into_iter, the map is consumed
        seen.sort_by_key(|x| x.0);

        assert_eq!(seen, (0..10).map(|i| (i, i)).collect::<Vec<_>>());

        // all values must be dropped exactly once
        assert_eq!(drops.get(), 10);
    }

    #[test]
    fn test_number_map_drain() {
        let drops = Rc::new(Cell::new(0));
        let mut m = NumberKeyMap::new();

        for i in 0..10 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        assert_eq!(drops.get(), 0);

        let mut seen = Vec::new();
        for (k, v) in m.drain() {
            seen.push((k, v.0));
        }

        // after drain, the map is consumed
        seen.sort_by_key(|x| x.0);

        assert_eq!(seen, (0..10).map(|i| (i, i)).collect::<Vec<_>>());

        drop(m);

        // all values must be dropped exactly once
        assert_eq!(drops.get(), 10);

        let mut m = NumberKeyMap::new();

        for i in 0..10 {
            m.insert(i, DropCounter(i, drops.clone())).unwrap();
        }

        let iter = m.drain();

        #[allow(clippy::drop_non_drop, reason = "It is tested here")]
        drop(iter);

        assert_eq!(drops.get(), 10);
    }
}
