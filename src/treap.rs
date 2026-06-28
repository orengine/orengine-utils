//! A randomized binary search tree with subtree-augmented filtering.
//!
//! This module provides [`Treap`], an augmented treap that combines
//! standard BST ordering (via [`TreapEntry::sorting_key`]) with
//! efficient filtered min/max queries (via [`TreapEntry::filtering_key`]).
//!
//! Each node stores the maximum filtering key in its subtree, allowing
//! [`pop_max_with_filter`](Treap::pop_max_with_filter) and related
//! methods to prune entire branches.
use crate::cheap_random::cheap_random_with_current_u32;
use crate::hints::{cold_path, unlikely, unreachable_hint, unwrap_or_bug_hint};
use crate::ArrayBuffer;
use alloc::boxed::Box;
use alloc::format;
use core::cmp::Ordering;
use core::fmt;
use core::fmt::Display;
use core::mem;
use core::num::NonZeroU32;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// Trait for entries that can be stored in a [`Treap`].
///
/// Implementors provide two types of keys:
/// - **Sorting Key**: Used for BST (binary search tree) ordering
/// - **Filtering Key**: Used for subtree pruning during range queries
///
/// # Examples
///
/// ```rust
/// use core::cmp::Ordering;
/// use orengine_utils::treap::TreapEntry;
///
/// #[derive(Clone, Copy)]
/// struct Point {
///     x: i32,
///     y: i32,
///     payload: i32,
/// }
///
/// impl TreapEntry for Point {
///     type SortingKey = i32;
///     type FilteringKey = i32;
///     type Value = i32;
///
///     fn sorting_key(&self) -> &Self::SortingKey {
///         &self.x
///     }
///
///     fn filtering_key(&self) -> &Self::FilteringKey {
///         &self.y
///     }
///
///     fn value(&self) -> &Self::Value {
///         &self.payload
///     }
///
///     fn value_mut(&mut self) -> &mut Self::Value {
///         &mut self.payload
///     }
/// }
/// ```
pub trait TreapEntry {
    /// Key type for BST ordering. Can be a tuple for compound keys.
    type SortingKey: Ord;

    /// Key type for filtering/pruning.
    type FilteringKey: Ord + Clone;

    /// Value type stored in the entry.
    type Value;

    /// Returns the sorting key for BST ordering.
    fn sorting_key(&self) -> &Self::SortingKey;

    /// Returns the filtering key for pruning decisions.
    fn filtering_key(&self) -> &Self::FilteringKey;

    /// Returns a shared reference to the underlying value.
    fn value(&self) -> &Self::Value;

    /// Returns an exclusive reference to the underlying value.
    fn value_mut(&mut self) -> &mut Self::Value;
}

/// Node in the [`Treap`].
///
/// It can be dereferenced into a shared reference to the [`TreapEntry`].
/// You can also use [`Node::neighbors`] to get an iterator over the neighbors of this node.
pub struct Node<E: TreapEntry> {
    entry: E,

    /// Maximum filter key in this subtree
    max_filter: E::FilteringKey,

    priority: u32,
    left: Option<NonNull<Node<E>>>,
    right: Option<NonNull<Node<E>>>,
    parent: Option<NonNull<Node<E>>>,
}

impl<E: TreapEntry> Node<E> {
    /// Returns a shared reference to the entry.
    #[inline]
    fn entry(&self) -> &E {
        &self.entry
    }

    /// Returns a mutable reference to the entry.
    #[inline]
    fn entry_mut(&mut self) -> &mut E {
        &mut self.entry
    }

    /// Returns an iterator over nodes reachable from `self` whose
    /// `filtering_key() >= filter`.
    ///
    /// Traversal visits both subtree descendants and ancestors,
    /// pruning branches where the subtree's `max_filter < filter`.
    ///
    /// `skip_right` skips the right subtree on the first step, useful when
    /// the caller has already consumed the greatest node (e.g., after
    /// [`Treap::peek_max_with_filter`]).
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<usize, usize, ()>>::new();
    ///
    /// for i in 1..=5 {
    ///     treap.set(BaseTreapEntry::new(i, i, ()));
    /// }
    ///
    /// let node = treap.peek_max_with_filter(&3).unwrap();
    ///
    /// let keys: Vec<_> = node.neighbors(&3, true) // `true` because we already know the greatest entry with `FilteringKey` >= 3
    ///     .map(|n| n.sorting_key)
    ///     .collect();
    ///
    /// assert_eq!(keys, vec![5, 4, 3]);
    /// ```
    #[allow(
        clippy::too_many_lines,
        reason = "There is only a slight excess here,\
         but moving the iterator out will ruin readability."
    )]
    pub fn neighbors<'treap>(
        &'treap self,
        filter: &'treap E::FilteringKey,
        skip_right: bool,
    ) -> impl Iterator<Item = &'treap Self> {
        /// An internal state machine for the neighbor iterator,
        /// controlling traversal direction through the treap.
        #[derive(Clone, Copy, PartialEq)]
        enum Phase {
            Start,              // Initial state for the node
            StartAndNextGoLeft, // Initial state for the node
            GoRight,            // Go to right child
            GoLeft,             // Go to left child
            GoUp,               // Return to parent
        }

        struct NeighborsIterator<'treap, E: TreapEntry> {
            current: NonNull<Node<E>>,
            filter: &'treap E::FilteringKey,
            // Depth relative to the start node.
            // 0: Ancestors or Start Node. > 0: Descendants.
            // -1 is used transiently to detect moving into an ancestor.
            depth: i32,
            phase: Phase,
        }

        impl<E: TreapEntry> NeighborsIterator<'_, E> {
            /// Checks if a subtree contains nodes meeting the filter threshold.
            fn check_subtree(
                node: Option<NonNull<Node<E>>>,
                filter: &E::FilteringKey,
            ) -> Option<NonNull<Node<E>>> {
                node.filter(|n| unsafe { n.as_ref().max_filter >= *filter })
            }
        }

        impl<'a, E: TreapEntry + 'a> Iterator for NeighborsIterator<'a, E> {
            type Item = &'a Node<E>;

            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    unsafe {
                        let node = self.current.as_ref();

                        match self.phase {
                            Phase::Start => {
                                // 1. Yield Current Node
                                self.phase = Phase::GoRight;

                                if node.entry.filtering_key() >= self.filter {
                                    return Some(node);
                                }
                            }

                            Phase::StartAndNextGoLeft => {
                                self.phase = Phase::GoLeft;

                                if node.entry.filtering_key() >= self.filter {
                                    return Some(node);
                                }
                            }

                            Phase::GoRight => {
                                // 2. Try Right Child
                                self.phase = Phase::GoLeft;

                                if let Some(right) = Self::check_subtree(node.right, self.filter) {
                                    self.current = right;
                                    self.depth += 1;
                                    self.phase = Phase::Start;
                                }
                            }

                            Phase::GoLeft => {
                                // 3. Try Left Child
                                self.phase = Phase::GoUp;

                                if let Some(left) = Self::check_subtree(node.left, self.filter) {
                                    self.current = left;
                                    self.depth += 1;
                                    self.phase = Phase::Start;
                                }
                            }

                            Phase::GoUp => {
                                // 4. Go Up
                                if let Some(parent_ptr) = node.parent {
                                    let parent = parent_ptr.as_ref();

                                    // Determine relationship BEFORE moving current pointer logic fully
                                    let is_right_child = parent.right == Some(self.current);

                                    self.current = parent_ptr;
                                    self.depth -= 1;

                                    if self.depth < 0 {
                                        // We just stepped into an Ancestor (relative to start)
                                        self.depth = 0;

                                        // Yield the ancestor if valid
                                        if parent.entry.filtering_key() >= self.filter {
                                            // After yielding, determine next step
                                            if is_right_child {
                                                self.phase = Phase::GoLeft;
                                            } else {
                                                self.phase = Phase::GoUp;
                                            }
                                            return Some(parent);
                                        }

                                        // The ancestor is invalid, but maybe another subtree is valid
                                        if is_right_child {
                                            self.phase = Phase::GoLeft;
                                        } else {
                                            self.phase = Phase::GoUp;
                                        }
                                    } else {
                                        // We are bubbling up inside the Start Node's subtree.
                                        // We visited this node and its Right child.
                                        // If we came from Right, we must now check Left.
                                        // If we came from Left, we are done with this node (Go Up).
                                    }

                                    if is_right_child {
                                        self.phase = Phase::GoLeft;
                                    } else {
                                        self.phase = Phase::GoUp;
                                    }
                                } else {
                                    // Root reached
                                    return None;
                                }
                            }
                        }
                    }
                }
            }
        }

        let initial_phase = if skip_right {
            Phase::StartAndNextGoLeft
        } else {
            Phase::Start
        };

        NeighborsIterator {
            current: NonNull::from(self),
            filter,
            depth: 0,
            phase: initial_phase,
        }
    }
}

impl<E: TreapEntry> Deref for Node<E> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.entry()
    }
}

impl<E: TreapEntry> DerefMut for Node<E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.entry_mut()
    }
}

/// A treap (tree and heap) data structure combining a binary search tree with randomized
/// heap-based balancing.
///
/// Supports efficient filtering via subtree augmentation with maximum filtering keys.
///
/// # Example
///
/// ```rust
/// use orengine_utils::treap::{Node, Treap, TreapEntry};
///
/// #[derive(Clone, Copy)]
/// struct Point {
///     x: i32,
///     y: i32,
///     payload: i32,
/// }
///
/// impl TreapEntry for Point {
///     type SortingKey = i32;
///     type FilteringKey = i32;
///     type Value = i32;
///
///     fn sorting_key(&self) -> &Self::SortingKey {
///         &self.x
///     }
///
///     fn filtering_key(&self) -> &Self::FilteringKey {
///         &self.y
///     }
///
///     fn value(&self) -> &Self::Value {
///         &self.payload
///     }
///
///     fn value_mut(&mut self) -> &mut Self::Value {
///         &mut self.payload
///     }
/// }
///
/// let mut treap = Treap::new();
///
/// treap.set(Point { x: 1, y: 5, payload: -1 });
/// treap.set(Point { x: 2, y: 4, payload: -2 });
/// treap.set(Point { x: 3, y: 3, payload: -3 });
/// treap.set(Point { x: 4, y: 2, payload: -4 });
/// treap.set(Point { x: 5, y: 1, payload: -5 });
///
/// let the_greatest: &Node<Point> = treap.peek_max_with_filter(&4).unwrap(); // Peeks the greatest by `x` entry with `y` >= 4
///
/// assert_eq!(the_greatest.x, 2);
///
/// // Iterator over points with `y` >= 4 around `the_greatest` to start from the greatest by `x`
/// // `true` for `skip_right` because we already know the greatest and want to search for `y <= the_greatest.y`.
/// let neighbors = the_greatest.neighbors(&4, true);
///
/// assert_eq!(neighbors.map(|point| point.payload).collect::<Vec<i32>>(), vec![-2, -1]);
/// ```
pub struct Treap<E: TreapEntry> {
    root: Option<NonNull<Node<E>>>,
    rng: NonZeroU32,
}

/// Node in the [`Treap`].
///
/// It can be dereferenced into a shared reference to the [`TreapEntry`], and it provides
/// a mutable reference to the underlying value by [`NodeMut::value_mut`].
///
/// You can also use [`Node::neighbors`] to get an iterator over the neighbors of this node.
///
/// And it can be used to remove the node from the treap by [`NodeMut::remove_from_treap`].
pub struct NodeMut<'handle, E: TreapEntry> {
    treap: &'handle mut Treap<E>,
    node_ptr: NonNull<Node<E>>,
}

impl<E: TreapEntry> NodeMut<'_, E> {
    /// Returns an exclusive reference to the value of this node.
    pub fn value_mut(&mut self) -> &mut E::Value {
        let node = unsafe { self.node_ptr.as_mut() };

        node.value_mut()
    }

    /// Removes this node from the associated [`Treap`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// unsafe { treap.add(BaseTreapEntry::new(1, 1, ())) };
    ///
    /// let mut node = treap.peek_max_with_filter_mut(&1).unwrap();
    /// if node.sorting_key > 0 { // Remove on condition, you can use the node not to use search it again
    ///     let entry = node.remove_from_treap();
    ///
    ///     assert_eq!(entry.sorting_key, 1);
    ///     assert!(treap.is_empty());
    /// }
    /// ```
    pub fn remove_from_treap(&mut self) -> E {
        self.treap.remove_node_by_ptr(self.node_ptr)
    }
}

impl<E: TreapEntry> Deref for NodeMut<'_, E> {
    type Target = Node<E>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.node_ptr.as_ref() }
    }
}

impl<E: TreapEntry> Treap<E> {
    /// Creates a new empty treap.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// assert!(treap.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            root: None,
            rng: unsafe { NonZeroU32::new_unchecked(1_406_868_647) },
        }
    }

    /// Returns `true` if the treap contains no entries.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// assert!(treap.is_empty());
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    ///
    /// assert!(!treap.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Recomputes the `max_filter` for `node` from its entry and children.
    /// Must be called after any structural change to `node`'s children.
    #[inline]
    fn update_augmentation_(node: &mut Node<E>) {
        let mut max_f = node.entry.filtering_key();

        if let Some(left_ptr) = node.left {
            let left = unsafe { left_ptr.as_ref() };
            if left.max_filter > *max_f {
                max_f = &left.max_filter;
            }
        }

        if let Some(right_ptr) = node.right {
            let right = unsafe { right_ptr.as_ref() };
            if right.max_filter > *max_f {
                max_f = &right.max_filter;
            }
        }

        node.max_filter = max_f.clone();
    }

    /// Recomputes the `max_filter` for the node at `node`.
    /// This is a pointer-based wrapper around [`update_augmentation_`].
    fn update_augmentation(mut node: NonNull<Node<E>>) {
        let node = unsafe { node.as_mut() };

        Self::update_augmentation_(node);
    }

    /// Performs a right rotation at the given node.
    fn rotate_right(&mut self, mut x: NonNull<Node<E>>) {
        let x_ref = unsafe { x.as_mut() };
        let mut y = unwrap_or_bug_hint(x_ref.left);
        let y_ref = unsafe { y.as_mut() };

        // Update x's left child
        x_ref.left = y_ref.right;
        if let Some(right_of_y) = y_ref.right {
            unsafe { (*right_of_y.as_ptr()).parent = Some(x) };
        }

        // Update y's parent
        y_ref.parent = x_ref.parent;

        // Update parent's child pointer
        match x_ref.parent {
            Some(p) => {
                let parent = unsafe { &mut *p.as_ptr() };
                if parent.left == Some(x) {
                    parent.left = Some(y);
                } else {
                    parent.right = Some(y);
                }
            }
            None => {
                self.root = Some(y);
            }
        }

        // Complete the rotation
        y_ref.right = Some(x);
        x_ref.parent = Some(y);

        // Update augmentations: only x and y changed their children
        Self::update_augmentation_(x_ref);
        Self::update_augmentation_(y_ref);

        // Update ancestors if needed
        if let Some(parent_ptr) = y_ref.parent {
            Self::update_augmentation(parent_ptr);
        }
    }

    /// Performs a left rotation at the given node.
    fn rotate_left(&mut self, mut x: NonNull<Node<E>>) {
        let x_ref = unsafe { x.as_mut() };
        let mut y = unwrap_or_bug_hint(x_ref.right);
        let y_ref = unsafe { y.as_mut() };

        // Update x's right child
        x_ref.right = y_ref.left;
        if let Some(left_of_y) = y_ref.left {
            unsafe { (*left_of_y.as_ptr()).parent = Some(x) };
        }

        // Update y's parent
        y_ref.parent = x_ref.parent;

        // Update parent's child pointer
        match x_ref.parent {
            Some(p) => {
                let parent = unsafe { &mut *p.as_ptr() };
                if parent.left == Some(x) {
                    parent.left = Some(y);
                } else {
                    parent.right = Some(y);
                }
            }
            None => {
                self.root = Some(y);
            }
        }

        // Complete the rotation
        y_ref.left = Some(x);
        x_ref.parent = Some(y);

        // Update augmentations: only x and y changed their children
        Self::update_augmentation_(x_ref);
        Self::update_augmentation_(y_ref);

        // Update ancestors if needed
        if let Some(parent_ptr) = y_ref.parent {
            Self::update_augmentation(parent_ptr);
        }
    }

    /// Moves `node` upward via rotations until the heap priority property
    /// (parent.priority >= child.priority) is restored.
    fn bubble_up(&mut self, node: NonNull<Node<E>>) {
        loop {
            let node_ref = unsafe { node.as_ref() };

            let Some(parent) = node_ref.parent else { break };

            let parent_ref = unsafe { parent.as_ref() };
            if parent_ref.priority >= node_ref.priority {
                break;
            }

            if parent_ref.left == Some(node) {
                self.rotate_right(parent);
            } else {
                self.rotate_left(parent);
            }
        }
    }

    /// Propagates updated `max_filter` values upward from `node` toward the
    /// root, stopping early once a node is found whose `max_filter` is
    /// greater than `deleted_filtering_key`.
    fn update_filter_to_root_after_deleting(
        mut node: NonNull<Node<E>>,
        deleted_filtering_key: &E::FilteringKey,
    ) {
        loop {
            let node_ref = unsafe { node.as_ref() };
            let parent = node_ref.parent;

            Self::update_augmentation(node);

            if let Some(parent_ptr) = parent {
                let parent_ref = unsafe { parent_ptr.as_ref() };
                if &parent_ref.max_filter == deleted_filtering_key {
                    node = parent_ptr;
                } else {
                    if cfg!(test) {
                        assert!(&parent_ref.max_filter > deleted_filtering_key);
                    }

                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Core insert/update implementation.
    ///
    /// When `ONLY_ADD` is `true` (used by [`add`](Treap::add)), panics in
    /// debug mode if the key already exists.  When `false` (used by
    /// [`set`](Treap::set)), replaces the existing entry and returns it.
    ///
    /// Returns `(node_ptr, old_entry)`.
    fn set_<const ONLY_ADD: bool>(&mut self, entry: E) -> (NonNull<Node<E>>, Option<E>) {
        let mut new_node = |entry: E| -> NonNull<Node<E>> {
            let max_filter = entry.filtering_key().clone();
            let node = Box::new(Node {
                entry,
                priority: cheap_random_with_current_u32(&mut self.rng),
                left: None,
                right: None,
                parent: None,
                max_filter,
            });

            unsafe { NonNull::new_unchecked(Box::into_raw(node)) }
        };

        // Handle empty tree
        if unlikely(self.root.is_none()) {
            let node_ptr = new_node(entry);

            self.root = Some(node_ptr);

            return (node_ptr, None);
        }

        let mut current_ = self.root;
        let mut prev_node: Option<NonNull<Node<E>>> = None;
        let mut is_left_child = false;

        unsafe {
            while let Some(mut current) = current_ {
                prev_node = Some(current);

                if entry.filtering_key() > &current.as_mut().max_filter {
                    current.as_mut().max_filter = entry.filtering_key().clone();
                }

                match (
                    current
                        .as_mut()
                        .entry
                        .sorting_key()
                        .cmp(entry.sorting_key()),
                    ONLY_ADD,
                ) {
                    (Ordering::Greater, _) => {
                        current_ = current.as_mut().left;
                        is_left_child = true;
                    }
                    (Ordering::Less, _) => {
                        current_ = current.as_mut().right;
                        is_left_child = false;
                    }
                    (Ordering::Equal, true) => {
                        if cfg!(debug_assertions) {
                            panic!("The method `add` can only adds entries, not update them");
                        } else {
                            unreachable_hint();
                        }
                    }
                    (Ordering::Equal, false) => {
                        let old_entry = mem::replace(&mut current.as_mut().entry, entry);
                        if unlikely(current.as_mut().filtering_key() < old_entry.filtering_key()) {
                            Self::update_augmentation(current);

                            if let Some(parent_ptr) = current.as_mut().parent {
                                Self::update_filter_to_root_after_deleting(
                                    parent_ptr,
                                    old_entry.filtering_key(),
                                );
                            }
                        } // else the treap augmentation is already correct

                        return (current, Some(old_entry));
                    }
                }
            }

            let mut node_ptr = new_node(entry);

            // Link to parent
            node_ptr.as_mut().parent = prev_node;
            if is_left_child {
                unwrap_or_bug_hint(prev_node).as_mut().left = Some(node_ptr);
            } else {
                unwrap_or_bug_hint(prev_node).as_mut().right = Some(node_ptr);
            }

            self.bubble_up(node_ptr);

            (node_ptr, None)
        }
    }

    /// Inserts `entry` into the treap, returning [`NodeMut`] with this entry.
    ///
    /// # Safety
    ///
    /// The treap must not yet contain an entry with the same `SortingKey`.
    /// In debug builds, violating this panics; in release builds it is UB.
    ///
    /// Because this function does not check if it already contains the entry,
    /// it is faster than [`Treap::set`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry, NodeMut};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    /// let node_mut: NodeMut<'_, BaseTreapEntry<u32, u32, ()>> = unsafe {
    ///     treap.add(BaseTreapEntry::new(42, 7, ()))
    /// };
    ///
    /// assert_eq!(treap.find(&42).unwrap().sorting_key, 42);
    /// ```
    pub unsafe fn add(&mut self, entry: E) -> NodeMut<'_, E> {
        let node_ptr = self.set_::<true>(entry).0;

        NodeMut {
            treap: self,
            node_ptr,
        }
    }

    /// Inserts `entry`, or replaces the existing entry with the same
    /// `SortingKey` if one exists.
    ///
    /// Returns <code>([NodeMut], Some(old_entry))</code> on a replacement, or
    /// <code>([NodeMut], None)</code> on an insertion.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, &str>>::new();
    ///
    /// let (_node_mut, old) = treap.set(BaseTreapEntry::new(1, 10, "first"));
    /// assert!(old.is_none());
    ///
    /// let (_node_mut, old) = treap.set(BaseTreapEntry::new(1, 20, "second"));
    /// assert_eq!(old.unwrap().value, "first");
    /// ```
    pub fn set(&mut self, entry: E) -> (NodeMut<'_, E>, Option<E>) {
        let (node_ptr, old_entry) = self.set_::<false>(entry);

        (
            NodeMut {
                treap: self,
                node_ptr,
            },
            old_entry,
        )
    }

    /// Rotates `node` downward until it becomes a leaf, preserving the heap
    /// property for all other nodes. Used as the first step of deletion.
    ///
    /// # Safety
    ///
    /// `node` must be a valid pointer to a node currently in this treap.
    unsafe fn rotate_down_to_leaf(&mut self, node: NonNull<Node<E>>) {
        loop {
            let node_ref = unsafe { node.as_ref() };
            let left_priority = node_ref
                .left
                .map_or(0, |l| unsafe { (*l.as_ptr()).priority });
            let right_priority = node_ref
                .right
                .map_or(0, |r| unsafe { (*r.as_ptr()).priority });

            if node_ref.left.is_none() && node_ref.right.is_none() {
                break;
            }

            if left_priority > right_priority {
                self.rotate_right(node);
            } else if right_priority > 0 {
                self.rotate_left(node);
            } else {
                break;
            }
        }
    }

    /// Removes a node from the treap by its pointer.
    fn remove_node_by_ptr(&mut self, node: NonNull<Node<E>>) -> E {
        let node_ptr = node;

        unsafe {
            self.rotate_down_to_leaf(node_ptr);
        }

        let node_ref = unsafe { node.as_ref() };

        if let Some(parent) = node_ref.parent {
            let parent_ref = unsafe { &mut *parent.as_ptr() };
            if parent_ref.left == Some(node_ptr) {
                parent_ref.left = None;
            } else {
                parent_ref.right = None;
            }

            Self::update_filter_to_root_after_deleting(parent, node_ref.filtering_key());
        } else {
            self.root = None;
        }

        unsafe { Box::from_raw(node_ptr.as_ptr()) }.entry
    }

    /// Removes and returns the entry with `sorting_key`, or `None` if absent.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(5, 5, ()));
    ///
    /// assert!(treap.remove_by_sorting_key(&5).is_some());
    /// assert!(treap.remove_by_sorting_key(&5).is_none());
    /// ```
    pub fn remove_by_sorting_key(&mut self, sorting_key: &E::SortingKey) -> Option<E> {
        let node = self.find_ptr_mut(sorting_key)?;

        Some(self.remove_node_by_ptr(node))
    }

    /// Returns a pointer to the node with the greatest sorting key, or `None`.
    fn find_max_ptr(&self) -> Option<NonNull<Node<E>>> {
        let mut current = self.root?;

        unsafe {
            while let Some(right) = (*current.as_ptr()).right {
                current = right;
            }
        }

        Some(current)
    }

    /// Removes and returns the entry with the greatest sorting key, or `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.pop_max().unwrap().sorting_key, 3);
    /// assert_eq!(treap.pop_max().unwrap().sorting_key, 1);
    /// ```
    pub fn pop_max(&mut self) -> Option<E> {
        let min_node = self.find_max_ptr()?;

        Some(self.remove_node_by_ptr(min_node))
    }

    /// Returns [`Node`] with the greatest sorting key without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.peek_max().unwrap().sorting_key, 3);
    /// assert_eq!(treap.peek_max().unwrap().sorting_key, 3);
    /// ```
    pub fn peek_max(&self) -> Option<&Node<E>> {
        self.find_max_ptr().map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns [`NodeMut`] with the greatest sorting key without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.peek_max_mut().unwrap().sorting_key, 3);
    /// assert_eq!(treap.peek_max_mut().unwrap().sorting_key, 3);
    ///
    /// let mut node_mut = treap.peek_max_mut().unwrap();
    /// if node_mut.sorting_key > 2 { // conditional remove
    ///     node_mut.remove_from_treap();
    /// }
    /// ```
    pub fn peek_max_mut(&mut self) -> Option<NodeMut<'_, E>> {
        let mut current = self.root?;

        unsafe {
            while let Some(right) = (*current.as_ptr()).right {
                current = right;
            }
        }

        Some(NodeMut {
            node_ptr: current,
            treap: self,
        })
    }

    /// Returns a pointer to the node with the smallest sorting key, or `None`.
    fn find_min_ptr(&self) -> Option<NonNull<Node<E>>> {
        let mut current = self.root?;

        unsafe {
            while let Some(left) = (*current.as_ptr()).left {
                current = left;
            }
        }

        Some(current)
    }

    /// Removes and returns the entry with the smallest sorting key, or `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.pop_min().unwrap().sorting_key, 1);
    /// assert_eq!(treap.pop_min().unwrap().sorting_key, 3);
    /// ```
    pub fn pop_min(&mut self) -> Option<E> {
        let min_node = self.find_min_ptr()?;

        Some(self.remove_node_by_ptr(min_node))
    }

    /// Returns [`Node`] with the smallest sorting key without removing it.
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.peek_min().unwrap().sorting_key, 1);
    /// assert_eq!(treap.peek_min().unwrap().sorting_key, 1);
    /// ```
    pub fn peek_min(&self) -> Option<&Node<E>> {
        self.find_min_ptr().map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns [`NodeMut`] with the smallest sorting key without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    ///
    /// assert_eq!(treap.peek_min_mut().unwrap().sorting_key, 1);
    /// assert_eq!(treap.peek_min_mut().unwrap().sorting_key, 1);
    ///
    /// let mut node_mut = treap.peek_min_mut().unwrap();
    /// if node_mut.sorting_key < 2 { // conditional remove
    ///     node_mut.remove_from_treap();
    /// }
    /// ```
    pub fn peek_min_mut(&mut self) -> Option<NodeMut<'_, E>> {
        let mut current = self.root?;

        unsafe {
            while let Some(left) = (*current.as_ptr()).left {
                current = left;
            }
        }

        Some(NodeMut {
            node_ptr: current,
            treap: self,
        })
    }

    /// Generic filtered search. When `FIND_MAX` is `true`, returns the
    /// greatest node whose `filtering_key() >= min_filter`; when `false`,
    /// the smallest such node. Prunes branches via the `max_filter` augmentation.
    fn find_with_filter<const FIND_MAX: bool>(
        &self,
        min_filter: &E::FilteringKey,
    ) -> Option<NonNull<Node<E>>> {
        let root = self.root?;
        if unlikely(unsafe { root.as_ref().max_filter < *min_filter }) {
            return None;
        }

        let mut current = root;

        loop {
            unsafe {
                let node_ref = current.as_ref();
                let (first_, second_) = if FIND_MAX {
                    (node_ref.right, node_ref.left)
                } else {
                    (node_ref.left, node_ref.right)
                };

                if let Some(first) = first_ {
                    if &(*first.as_ptr()).max_filter >= min_filter {
                        current = first;

                        continue;
                    }
                }

                if node_ref.entry.filtering_key() >= min_filter {
                    return Some(current);
                }

                if let Some(second) = second_ {
                    if cfg!(test) {
                        assert!(
                            &(*second.as_ptr()).max_filter >= min_filter,
                            "the second node max_filter is not >= min_filter"
                        );
                    }

                    current = second;
                } else {
                    unreachable_hint();
                }
            }
        }
    }

    /// Finds the maximum node that satisfies `filtering_key() >= min_filter`.
    ///
    /// Uses subtree augmentation to prune branches.
    fn find_max_with_filter(&self, min_filter: &E::FilteringKey) -> Option<NonNull<Node<E>>> {
        Self::find_with_filter::<true>(self, min_filter)
    }

    /// Finds the minimum node that satisfies `filtering_key() >= min_filter`.
    ///
    /// Uses subtree augmentation to prune branches.
    fn find_min_with_filter(&self, min_filter: &E::FilteringKey) -> Option<NonNull<Node<E>>> {
        Self::find_with_filter::<false>(self, min_filter)
    }

    /// Removes and returns the entry with the greatest sorting key that satisfies
    /// `filtering_key() >= min_filter`, or `None` if no such entry exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.pop_max_with_filter(&5).unwrap().sorting_key, 2);
    /// assert_eq!(treap.pop_max_with_filter(&5).unwrap().sorting_key, 1);
    /// assert!(treap.pop_max_with_filter(&5).is_none());
    /// ```
    pub fn pop_max_with_filter(&mut self, min_filter: &E::FilteringKey) -> Option<E> {
        let node = self.find_max_with_filter(min_filter)?;

        Some(self.remove_node_by_ptr(node))
    }

    /// Returns [`Node`] with the greatest sorting key satisfying
    /// `filtering_key() >= min_filter`, without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.peek_max_with_filter(&5).unwrap().sorting_key, 2);
    /// assert_eq!(treap.peek_max_with_filter(&5).unwrap().sorting_key, 2);
    /// ```
    pub fn peek_max_with_filter(&self, min_filter: &E::FilteringKey) -> Option<&Node<E>> {
        self.find_max_with_filter(min_filter)
            .map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns [`NodeMut`] with the greatest sorting key satisfying
    /// `filtering_key() >= min_filter`, without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.peek_max_with_filter_mut(&5).unwrap().sorting_key, 2);
    /// assert_eq!(treap.peek_max_with_filter_mut(&5).unwrap().sorting_key, 2);
    ///
    /// let mut node_mut = treap.peek_max_with_filter_mut(&5).unwrap();
    /// if node_mut.sorting_key > 2 { // conditional remove
    ///     node_mut.remove_from_treap();
    /// }
    /// ```
    pub fn peek_max_with_filter_mut(
        &mut self,
        min_filter: &E::FilteringKey,
    ) -> Option<NodeMut<'_, E>> {
        self.find_max_with_filter(min_filter).map(|ptr| NodeMut {
            node_ptr: ptr,
            treap: self,
        })
    }

    /// Removes and returns the entry with the smallest sorting key that satisfies
    /// `filtering_key() >= min_filter`, or `None` if no such entry exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.pop_min_with_filter(&5).unwrap().sorting_key, 1);
    /// assert_eq!(treap.pop_min_with_filter(&5).unwrap().sorting_key, 2);
    /// assert!(treap.pop_min_with_filter(&5).is_none());
    /// ```
    pub fn pop_min_with_filter(&mut self, min_filter: &E::FilteringKey) -> Option<E> {
        let node = self.find_min_with_filter(min_filter)?;

        Some(self.remove_node_by_ptr(node))
    }

    /// Returns [`Node`] with the smallest sorting key satisfying
    /// `filtering_key() >= min_filter`, without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.peek_min_with_filter(&5).unwrap().sorting_key, 1);
    /// assert_eq!(treap.peek_min_with_filter(&5).unwrap().sorting_key, 1);
    /// ```
    pub fn peek_min_with_filter(&self, min_filter: &E::FilteringKey) -> Option<&Node<E>> {
        self.find_min_with_filter(min_filter)
            .map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns [`NodeMut`] with the smallest sorting key satisfying
    /// `filtering_key() >= min_filter`, without removing it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 10, ()));
    /// treap.set(BaseTreapEntry::new(2, 5, ()));
    /// treap.set(BaseTreapEntry::new(3, 1, ()));
    ///
    /// assert_eq!(treap.peek_min_with_filter_mut(&5).unwrap().sorting_key, 1);
    /// assert_eq!(treap.peek_min_with_filter_mut(&5).unwrap().sorting_key, 1);
    ///
    /// let mut node_mut = treap.peek_min_with_filter_mut(&5).unwrap();
    /// if node_mut.sorting_key < 2 { // conditional remove
    ///     node_mut.remove_from_treap();
    /// }
    /// ```
    pub fn peek_min_with_filter_mut(
        &mut self,
        min_filter: &E::FilteringKey,
    ) -> Option<NodeMut<'_, E>> {
        self.find_min_with_filter(min_filter).map(|ptr| NodeMut {
            node_ptr: ptr,
            treap: self,
        })
    }

    /// Returns an in-order iterator over all nodes from the smallest to the greatest sorting key.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{Treap, BaseTreapEntry};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, ()>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(3, 3, ()));
    /// treap.set(BaseTreapEntry::new(1, 1, ()));
    /// treap.set(BaseTreapEntry::new(2, 2, ()));
    ///
    /// let keys: Vec<_> = treap.iter().map(|n| n.sorting_key).collect();
    /// assert_eq!(keys, vec![1, 2, 3]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &Node<E>> {
        /// In-order iterator over [`Treap`] nodes, from the smallest to the greatest sorting key.
        pub struct Iter<'treap, E: TreapEntry> {
            current: Option<NonNull<Node<E>>>,
            _marker: core::marker::PhantomData<&'treap Treap<E>>,
        }

        impl<'treap, E: TreapEntry> Iterator for Iter<'treap, E> {
            type Item = &'treap Node<E>;

            fn next(&mut self) -> Option<Self::Item> {
                let curr = self.current?;

                let next_node;

                if let Some(node) = unsafe { curr.as_ref() }.right {
                    let mut temp = node;

                    while let Some(left) = unsafe { temp.as_ref() }.left {
                        temp = left;
                    }

                    next_node = Some(temp);
                } else {
                    let mut temp = curr;

                    loop {
                        if let Some(parent) = unsafe { temp.as_ref() }.parent {
                            let is_left_child = unsafe { parent.as_ref() }.left == Some(temp);

                            if is_left_child {
                                next_node = Some(parent);

                                break;
                            }

                            temp = parent;
                        } else {
                            next_node = None;

                            break;
                        }
                    }
                }

                let val = unsafe { curr.as_ref() };

                self.current = next_node;

                Some(val)
            }
        }

        let mut current = self.root;

        if let Some(node) = current {
            let mut curr = node;

            while let Some(left) = unsafe { curr.as_ref() }.left {
                curr = left;
            }

            current = Some(curr);
        }

        Iter {
            current,
            _marker: core::marker::PhantomData,
        }
    }

    /// Validate treap structure (for testing).
    #[cfg(test)]
    pub(crate) fn validate(&self)
    where
        E::FilteringKey: core::fmt::Debug,
    {
        if self.root.is_none() {
            return;
        }

        Self::validate_helper(self.root.unwrap(), None, None, None);
    }

    #[cfg(test)]
    fn validate_helper(
        node: NonNull<Node<E>>,
        min_key: Option<&E::SortingKey>,
        max_key: Option<&E::SortingKey>,
        parent: Option<NonNull<Node<E>>>,
    ) where
        E::FilteringKey: core::fmt::Debug,
    {
        let node_ref = unsafe { node.as_ref() };

        assert_eq!(node_ref.parent, parent);

        // BST property
        if let Some(min) = min_key {
            assert!(node_ref.entry.sorting_key() > min);
        }
        if let Some(max) = max_key {
            assert!(node_ref.entry.sorting_key() <= max);
        }

        // Heap property
        if let Some(p) = parent {
            assert!(unsafe { p.as_ref() }.priority >= node_ref.priority);
        }

        // Augmentation
        let mut expected_max = node_ref.entry.filtering_key();

        if let Some(left) = node_ref.left {
            Self::validate_helper(
                left,
                min_key,
                Some(node_ref.entry.sorting_key()),
                Some(node),
            );

            let left_ref = unsafe { left.as_ref() };
            if &left_ref.max_filter > expected_max {
                expected_max = &left_ref.max_filter;
            }
        }

        if let Some(right) = node_ref.right {
            Self::validate_helper(
                right,
                Some(node_ref.entry.sorting_key()),
                max_key,
                Some(node),
            );

            let right_ref = unsafe { right.as_ref() };
            if &right_ref.max_filter > expected_max {
                expected_max = &right_ref.max_filter;
            }
        }

        assert_eq!(expected_max, &node_ref.max_filter);
    }
}

macro_rules! generate_find_ptr {
    (
        $name:ident,
        $self_type:ty,
        $self_name:ident,
        $current_ptr_name:ident,
        $get_current_block:block
    ) => {
        fn $name($self_name: $self_type, key: &E::SortingKey) -> Option<NonNull<Node<E>>> {
            let mut $current_ptr_name = $self_name.root?;

            unsafe {
                loop {
                    let current_ref = $get_current_block;

                    match current_ref.entry.sorting_key().cmp(key) {
                        Ordering::Equal => return Some(NonNull::from(current_ref)),
                        Ordering::Greater => {
                            $current_ptr_name = current_ref.left?;
                        }
                        Ordering::Less => {
                            $current_ptr_name = current_ref.right?;
                        }
                    }
                }
            }
        }
    };
}

impl<E: TreapEntry> Treap<E> {
    generate_find_ptr!(find_ptr_, &Self, self_, current, { current.as_ref() });
    generate_find_ptr!(find_ptr_mut_, &mut Self, self_, current, {
        current.as_mut()
    });

    /// Returns a raw shared pointer to the node with the given sorting key,
    /// or `None` if absent.
    ///
    /// The pointer is valid until the node is removed or the treap is dropped.
    fn find_ptr(&self, key: &E::SortingKey) -> Option<NonNull<Node<E>>> {
        Self::find_ptr_(self, key)
    }

    /// Returns a raw mutable pointer to the node with the given sorting key,
    /// or `None` if absent.
    ///
    /// See [`find_ptr`](Treap::find_ptr) for a usage example.
    fn find_ptr_mut(&mut self, key: &E::SortingKey) -> Option<NonNull<Node<E>>> {
        Self::find_ptr_mut_(self, key)
    }

    /// Returns [`Node`] with the given sorting key, or `None` if absent.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::{BaseTreapEntry, Treap};
    ///
    /// let mut treap = Treap::<BaseTreapEntry<u32, u32, &str>>::new();
    ///
    /// treap.set(BaseTreapEntry::new(1, 1, "hello"));
    ///
    /// assert_eq!(treap.find(&1).unwrap().value, "hello");
    /// ```
    pub fn find(&self, key: &E::SortingKey) -> Option<&Node<E>> {
        let ptr = self.find_ptr(key)?;

        Some(unsafe { ptr.as_ref() })
    }

    /// Returns [`NodeMut`] with the given sorting key, or `None` if absent.
    ///
    /// See [`find`](Treap::find) for a usage example.
    pub fn find_mut(&mut self, key: &E::SortingKey) -> Option<NodeMut<'_, E>> {
        let ptr = self.find_ptr_mut(key)?;

        Some(NodeMut {
            treap: self,
            node_ptr: ptr,
        })
    }
}

impl<E: TreapEntry> Default for Treap<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: TreapEntry + Display> Display for Treap<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print_node<E: TreapEntry + Display>(
            f: &mut fmt::Formatter<'_>,
            node: NonNull<Node<E>>,
            prefix: &str,
            is_last: bool,
            is_left: bool,
        ) -> fmt::Result {
            let n = unsafe { node.as_ref() };

            // 1. Print the connector
            // If it's the last child, use └──, else use ├──
            let connector = if is_last { "└── " } else { "├── " };
            let order_msg = if is_left { "Left" } else { "Right" };
            write!(f, "{prefix}{connector}{order_msg} ")?;

            // 2. Print the node value
            writeln!(f, "{}", n.entry)?;

            // 3. Prepare prefix for children
            // If this node was the last child, the extension for children is spaces (don't draw line).
            // If it wasn't the last child, the extension is a vertical bar (to connect to the sibling below).
            let child_prefix = if is_last {
                format!("{prefix}    ") // 4 spaces
            } else {
                format!("{prefix}│   ") // Pipe + 3 spaces
            };

            // 4. Recurse for children
            let has_right = n.right.is_some();

            // Print Left Child
            // It is the "last" child visible at this level if there is NO right child.
            if let Some(left) = &n.left {
                print_node(f, *left, &child_prefix, !has_right, true)?;
            }

            // Print Right Child
            // It is always the "last" child if it exists.
            if let Some(right) = &n.right {
                print_node(f, *right, &child_prefix, true, false)?;
            }

            Ok(())
        }

        match &self.root {
            Some(node) => {
                // We treat the root specially: it has no prefix and no "is_last" status
                // in the traditional sense, but we start the recursion here.
                writeln!(f, "{}", unsafe { node.as_ref() }.entry)?;

                // Print children. We need to know if a child is the "last" one
                // to draw the correct branch shape (└── vs. ├──).
                let rb = unsafe { node.as_ref() };

                // Process Right Child (visually "top" branch in rotated view, or second in text)
                // Typically we print Left then Right.
                // In a text diagram, often Right is printed first to keep it "upright"
                // or Left first to follow reading order.
                // Let's stick to the standard Left-then-Right reading order for the diagram.

                let has_right = rb.right.is_some();

                if let Some(left) = &rb.left {
                    print_node(f, *left, "", !has_right, true)?;
                }
                if let Some(right) = &rb.right {
                    print_node(f, *right, "", true, false)?;
                }

                Ok(())
            }
            None => write!(f, "Empty Tree"),
        }
    }
}

impl<E: TreapEntry> Drop for Treap<E> {
    fn drop(&mut self) {
        fn drop_subtree<E: TreapEntry>(mut current: Option<NonNull<Node<E>>>) {
            // Stack memory is almost free to allocate, so we can allocate
            // 256 * 8 = 2KB of stack memory and do not care about the performance.
            // But the treap can become a linked-list if we are extremely unlucky,
            // so we need to be careful about the stack overflow.
            // We handle it below.
            let mut stack = ArrayBuffer::<_, 256>::new();
            let mut last_freed: Option<NonNull<Node<E>>> = None;

            while current.is_some() || !stack.is_empty() {
                // Dive as far left as possible
                while let Some(n) = current {
                    let res = stack.push(n);
                    if let Err(left) = res {
                        // Drop the subtree in a new function with a new stack

                        cold_path();

                        drop_subtree(Some(left));
                    } else {
                        current = unsafe { n.as_ref() }.left;
                    }
                }

                let &top = stack.last().unwrap();
                let node_ref = unsafe { top.as_ref() };

                // If there's an unprocessed right child, go there
                if node_ref.right.is_some() && node_ref.right != last_freed {
                    current = node_ref.right;
                } else {
                    // Both children are done, free this node

                    stack.pop();

                    last_freed = Some(top);

                    let _ = unsafe { Box::from_raw(top.as_ptr()) };
                }
            }
        }

        drop_subtree(self.root);
    }
}

/// A ready-to-use [`TreapEntry`] implementation wrapping a sorting key,
/// filtering key, and a value.
///
/// Use this when you don't need a custom entry type.
///
/// # Example
///
/// ```rust
/// use orengine_utils::treap::{BaseTreapEntry, Treap};
///
/// let mut treap = Treap::<BaseTreapEntry<u32, u32, &str>>::new();
///
/// treap.set(BaseTreapEntry::new(1, 10, "hello"));
///
/// assert_eq!(treap.find(&1).unwrap().value, "hello");
/// ```
pub struct BaseTreapEntry<SortingKey: Ord, FilteringKey: Ord + Clone, V> {
    pub sorting_key: SortingKey,
    pub filtering_key: FilteringKey,
    pub value: V,
}

impl<SortingKey: Ord, FilteringKey: Ord + Clone, V> BaseTreapEntry<SortingKey, FilteringKey, V> {
    /// Creates a new [`BaseTreapEntry`] with the given sorting key, filtering key, and value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use orengine_utils::treap::BaseTreapEntry;
    ///
    /// let entry = BaseTreapEntry::new(42u32, 7u32, "payload");
    ///
    /// assert_eq!(entry.sorting_key, 42);
    /// assert_eq!(entry.filtering_key, 7);
    /// assert_eq!(entry.value, "payload");
    /// ```
    #[inline]
    pub fn new(sorting_key: SortingKey, filtering_key: FilteringKey, value: V) -> Self {
        Self {
            sorting_key,
            filtering_key,
            value,
        }
    }
}

impl<SortingKey: Ord, FilteringKey: Ord + Clone, V> TreapEntry
    for BaseTreapEntry<SortingKey, FilteringKey, V>
{
    type SortingKey = SortingKey;
    type FilteringKey = FilteringKey;
    type Value = V;

    #[inline]
    fn sorting_key(&self) -> &Self::SortingKey {
        &self.sorting_key
    }

    #[inline]
    fn filtering_key(&self) -> &Self::FilteringKey {
        &self.filtering_key
    }

    #[inline]
    fn value(&self) -> &Self::Value {
        &self.value
    }

    #[inline]
    fn value_mut(&mut self) -> &mut Self::Value {
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::string::ToString;
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::iter::from_fn;

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct TestEntry {
        primary: i32,
        filter: i32,
        value: String,
    }

    fn generate_filtering_key(primary: i32) -> i32 {
        primary % 50
    }

    impl TestEntry {
        fn new(primary: i32, filter: i32, value: &str) -> Self {
            Self {
                primary,
                filter,
                value: value.to_string(),
            }
        }
    }

    impl TreapEntry for TestEntry {
        type SortingKey = i32;
        type FilteringKey = i32;
        type Value = String;

        fn sorting_key(&self) -> &Self::SortingKey {
            &self.primary
        }

        fn filtering_key(&self) -> &Self::FilteringKey {
            &self.filter
        }

        fn value(&self) -> &Self::Value {
            &self.value
        }

        fn value_mut(&mut self) -> &mut Self::Value {
            &mut self.value
        }
    }

    impl Display for TestEntry {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({}, {})", self.primary, self.filter)
        }
    }

    #[test]
    #[cfg(not(feature = "no_std"))]
    fn test_as_tree() {
        let mut treap = Treap::new();

        for i in 0..100 {
            unsafe {
                treap.add(TestEntry::new(
                    i,
                    generate_filtering_key(i),
                    &format!("item_{i}"),
                ));
            };

            let item = treap.find(&i).unwrap();

            assert_eq!(item.primary, i);
            assert_eq!(item.filter, generate_filtering_key(i));
            assert_eq!(&item.value, &format!("item_{i}"));
        }

        for i in 0..100 {
            let item = treap.find(&i).unwrap();

            assert_eq!(item.primary, i);
            assert_eq!(item.filter, generate_filtering_key(i));
            assert_eq!(&item.value, &format!("item_{i}"));
        }

        for i in 0..100 {
            if i % 2 == 0 {
                let delta = if i % 4 == 0 { 10 } else { -10 };
                treap.set(TestEntry::new(
                    i,
                    generate_filtering_key(i + delta),
                    &format!("updated_item_{i}"),
                ));

                let item = treap.find(&i).unwrap();

                assert_eq!(item.primary, i);
                assert_eq!(item.filter, generate_filtering_key(i + delta));
                assert_eq!(&item.value, &format!("updated_item_{i}"));
            } else {
                let entry = treap.remove_by_sorting_key(&i).unwrap();

                assert_eq!(entry.primary, i);
                assert_eq!(entry.filter, generate_filtering_key(i));
                assert_eq!(&entry.value, &format!("item_{i}"));
            }

            treap.validate();
        }

        println!("{treap}");
    }

    #[test]
    fn test_pop_best() {
        let mut treap: Treap<TestEntry> = Treap::new();

        treap.set(TestEntry::new(3, 30, "three"));
        treap.set(TestEntry::new(1, 10, "one"));
        treap.set(TestEntry::new(2, 20, "two"));

        treap.validate();

        let e = treap.peek_min().unwrap();
        assert_eq!(e.primary, 1);
        assert_eq!(e.value, "one");

        treap.validate();

        let e = treap.pop_min().unwrap();
        assert_eq!(e.primary, 1);
        assert_eq!(e.value, "one");

        treap.validate();

        let e = treap.peek_min().unwrap();
        assert_eq!(e.primary, 2);
        assert_eq!(e.value, "two");

        treap.validate();

        let e = treap.pop_min().unwrap();
        assert_eq!(e.primary, 2);

        treap.validate();

        let e = treap.peek_min().unwrap();
        assert_eq!(e.primary, 3);

        treap.validate();

        let e = treap.pop_min().unwrap();
        assert_eq!(e.primary, 3);

        treap.validate();

        treap.set(TestEntry::new(3, 30, "three"));
        treap.set(TestEntry::new(1, 10, "one"));
        treap.set(TestEntry::new(2, 20, "two"));

        let e = treap.pop_max().unwrap();
        assert_eq!(e.primary, 3);
        assert_eq!(e.value, "three");

        treap.validate();

        let e = treap.pop_max().unwrap();
        assert_eq!(e.primary, 2);

        treap.validate();

        let e = treap.pop_max().unwrap();
        assert_eq!(e.primary, 1);

        treap.validate();

        assert!(treap.pop_max().is_none());
    }

    #[test]
    fn test_remove_by_pointer() {
        let mut treap: Treap<TestEntry> = Treap::new();
        let mut to_remove = Vec::new();

        for i in 0..100 {
            if i % 2 == 0 {
                to_remove.push(
                    *treap
                        .set(TestEntry::new(i, i * 10, &format!("even_{i}")))
                        .0
                        .sorting_key(),
                );
            } else {
                unsafe { treap.add(TestEntry::new(i, i * 10, &format!("odd_{i}"))) };
            }
        }

        assert_eq!(treap.iter().count(), 100);

        for sorting_key in to_remove {
            let e = if sorting_key % 2 == 0 {
                treap.remove_by_sorting_key(&sorting_key).unwrap()
            } else {
                treap.find_mut(&sorting_key).unwrap().remove_from_treap()
            };

            assert!(e.value.starts_with("even_"));

            treap.validate();
        }

        assert_eq!(treap.iter().count(), 50);

        treap.validate();
    }

    #[test]
    fn test_filtering_max() {
        let mut treap: Treap<TestEntry> = Treap::new();

        treap.set(TestEntry::new(1, 20, "a"));
        treap.set(TestEntry::new(2, 18, "b"));
        treap.set(TestEntry::new(3, 15, "c"));
        treap.set(TestEntry::new(4, 13, "d"));
        treap.set(TestEntry::new(5, 10, "e"));

        assert_eq!(treap.peek_max_with_filter(&15).unwrap().primary, 3);
        assert_eq!(treap.peek_max_with_filter(&20).unwrap().primary, 1);
        assert_eq!(treap.peek_max_with_filter(&16).unwrap().primary, 2);
        assert_eq!(treap.peek_max_with_filter(&1).unwrap().primary, 5);
        assert_eq!(treap.peek_max_with_filter(&11).unwrap().primary, 4);

        assert!(treap.peek_max_with_filter(&21).is_none());

        assert_eq!(
            &from_fn(|| treap.pop_max_with_filter(&15).map(|e| e.primary)).collect::<Vec<_>>(),
            &[3, 2, 1]
        );

        assert_eq!(treap.iter().count(), 2);

        treap.validate();
    }

    #[test]
    fn test_filtering_min() {
        let mut treap: Treap<TestEntry> = Treap::new();

        treap.set(TestEntry::new(1, 10, "a"));
        treap.set(TestEntry::new(2, 13, "b"));
        treap.set(TestEntry::new(3, 15, "c"));
        treap.set(TestEntry::new(4, 18, "d"));
        treap.set(TestEntry::new(5, 20, "e"));

        assert_eq!(treap.peek_min_with_filter(&15).unwrap().primary, 3);
        assert_eq!(treap.peek_min_with_filter(&20).unwrap().primary, 5);
        assert_eq!(treap.peek_min_with_filter(&16).unwrap().primary, 4);
        assert_eq!(treap.peek_min_with_filter(&1).unwrap().primary, 1);
        assert_eq!(treap.peek_min_with_filter(&12).unwrap().primary, 2);

        assert!(treap.peek_min_with_filter(&21).is_none());

        assert_eq!(
            &from_fn(|| treap.pop_min_with_filter(&15).map(|e| e.primary)).collect::<Vec<_>>(),
            &[3, 4, 5]
        );

        assert_eq!(treap.iter().count(), 2);

        treap.validate();
    }

    #[test]
    #[cfg(not(feature = "no_std"))]
    fn test_neighbors() {
        let mut treap: Treap<TestEntry> = Treap::new();

        // region filling

        treap.set(TestEntry::new(1, 1, "a"));
        treap.set(TestEntry::new(3, 2, "c"));
        treap.set(TestEntry::new(2, 10, "b"));
        treap.set(TestEntry::new(4, 4, "d"));
        treap.set(TestEntry::new(5, 5, "e"));
        treap.set(TestEntry::new(6, 6, "e"));
        treap.set(TestEntry::new(7, 7, "e"));
        treap.set(TestEntry::new(8, 8, "e"));
        treap.set(TestEntry::new(9, 9, "e"));
        treap.set(TestEntry::new(10, 3, "e"));
        treap.set(TestEntry::new(11, 3, "e"));
        treap.set(TestEntry::new(12, 0, "e"));
        treap.set(TestEntry::new(13, 0, "e"));
        treap.set(TestEntry::new(14, 1, "e"));
        let node15_key = *treap.set(TestEntry::new(15, 3, "e")).0.sorting_key();
        treap.set(TestEntry::new(16, 100, "e"));
        treap.set(TestEntry::new(17, 1, "e"));
        treap.set(TestEntry::new(18, 1, "e"));
        treap.set(TestEntry::new(19, 1, "e"));

        // endregion

        println!("Treap now: \n{treap}");

        // Treap now:
        // (17, 1)
        // ├── Left (10, 3)
        // │   ├── Left (6, 6)
        // │   │   ├── Left (3, 2)
        // │   │   │   ├── Left (1, 1)
        // │   │   │   │   └── Right (2, 10)
        // │   │   │   └── Right (5, 5)
        // │   │   │       └── Left (4, 4)
        // │   │   └── Right (9, 9)
        // │   │       └── Left (7, 7)
        // │   │           └── Right (8, 8)
        // │   └── Right (15, 3)
        // │       ├── Left (13, 0)
        // │       │   ├── Left (12, 0)
        // │       │   │   └── Left (11, 3)
        // │       │   └── Right (14, 1)
        // │       └── Right (16, 100)
        // └── Right (18, 1)
        //     └── Right (19, 1)

        let node15 = treap.find(&node15_key).unwrap();

        let neighbors: Vec<_> = node15
            .neighbors(&4, false)
            .map(|n| n.entry.primary)
            .collect();

        assert_eq!(neighbors, alloc::vec![16, 6, 9, 7, 8, 5, 4, 2]);

        let neighbors: Vec<_> = node15
            .neighbors(&4, true)
            .map(|n| n.entry.primary)
            .collect();

        assert_eq!(neighbors, alloc::vec![6, 9, 7, 8, 5, 4, 2]);
    }

    #[test]
    fn many_items() {
        const N: usize = if !cfg!(miri) { 3000 } else { 100 };

        let mut state = NonZeroU32::new(1).unwrap();
        let mut random = || {
            cheap_random_with_current_u32(&mut state);

            #[allow(clippy::cast_possible_wrap, reason = "It is fine here")]
            {
                state.get() as i32
            }
        };
        let mut treap = Treap::new();
        let mut inserted_nodes_keys = Vec::with_capacity(N);

        for _ in 0..N {
            inserted_nodes_keys.push(
                *treap
                    .set(TestEntry::new(
                        random(),
                        generate_filtering_key(random()),
                        "e",
                    ))
                    .0
                    .sorting_key(),
            );

            treap.validate();

            if random() % 5 == 0 {
                #[allow(clippy::cast_sign_loss, reason = "It is fine here")]
                treap
                    .find_mut(
                        &inserted_nodes_keys.remove(random() as usize % inserted_nodes_keys.len()),
                    )
                    .unwrap()
                    .remove_from_treap();

                treap.validate();
            }
        }
    }

    #[test]
    #[cfg(not(feature = "no_std"))]
    fn insert_and_remove_rand() {
        const N: usize = if !cfg!(miri) { 2000 } else { 20 };

        let mut state =
            NonZeroU32::new((crate::instant::OrengineInstant::now().into_u64() % 1000) as u32 + 1)
                .unwrap();

        for _ in 0..10 {
            let mut key_value_pairs = std::collections::HashMap::with_capacity(N);
            let mut tree = Treap::new();
            #[allow(clippy::cast_possible_truncation, reason = "False positive.")]
            #[allow(clippy::cast_possible_wrap, reason = "False positive.")]
            let mut rand_i32 =
                || cheap_random_with_current_u32(&mut state).cast_signed() % (N as i32 * 9 / 10);

            for i in 0..N {
                let key = rand_i32();
                let value_fn = || format!("{i}");

                if let Some(old_value) = key_value_pairs.insert(key, value_fn()) {
                    assert_eq!(
                        tree.find(&key).map(|v: &Node<TestEntry>| &v.value),
                        Some(&old_value)
                    );

                    let old_from_tree = tree.set(TestEntry::new(key, rand_i32(), &value_fn())).1;

                    assert!(old_from_tree.is_some());
                    assert_eq!(old_from_tree.unwrap().value, old_value);
                } else {
                    let r = rand_i32();
                    if r % 2 == 0 {
                        unsafe { tree.add(TestEntry::new(key, rand_i32(), &value_fn())) };
                    } else {
                        tree.set(TestEntry::new(key, rand_i32(), &value_fn()));
                    }
                }

                tree.validate();

                if rand_i32() % 5 == 0 {
                    let mut slice = key_value_pairs.iter().take(5).map(|(k, _v)| *k);
                    let mut wait = rand_i32() % 5;

                    while wait > 0 {
                        if slice.next().is_none() {
                            break;
                        }

                        wait -= 1;
                    }

                    if let Some(key) = slice.next() {
                        let value = key_value_pairs.remove(&key).unwrap();
                        let value_from_tree = tree.remove_by_sorting_key(&key);

                        assert!(value_from_tree.is_some());
                        assert_eq!(value_from_tree.unwrap().value, value);
                    }

                    tree.validate();
                }
            }
        }
    }

    #[test]
    #[cfg(not(feature = "no_std"))]
    fn test_drop() {
        use core::cell::Cell;

        thread_local! {
            static SORTING_KEY_DROP_COUNTER: Cell<usize> = const { Cell::new(0) };
            static VALUE_DROP_COUNTER: Cell<usize> = const { Cell::new(0) };
        }

        #[derive(Eq, PartialEq, Ord, PartialOrd)]
        struct SortingKeyWrapper(usize);

        impl Drop for SortingKeyWrapper {
            fn drop(&mut self) {
                SORTING_KEY_DROP_COUNTER.replace(SORTING_KEY_DROP_COUNTER.get() + 1);
            }
        }

        #[allow(dead_code, reason = "This value is used for debugging")]
        struct ValueWrapper(usize);

        impl Drop for ValueWrapper {
            fn drop(&mut self) {
                VALUE_DROP_COUNTER.replace(VALUE_DROP_COUNTER.get() + 1);
            }
        }

        let mut treap = Treap::<BaseTreapEntry<SortingKeyWrapper, usize, ValueWrapper>>::new();

        for i in 0..10 {
            let entry = BaseTreapEntry {
                sorting_key: SortingKeyWrapper(i),
                filtering_key: i,
                value: ValueWrapper(i),
            };

            if i % 2 == 0 {
                unsafe { treap.add(entry) };
            } else {
                treap.set(entry);
            }
        }

        assert_eq!(SORTING_KEY_DROP_COUNTER.get(), 0);
        assert_eq!(VALUE_DROP_COUNTER.get(), 0);

        for i in 0..10 {
            let entry = BaseTreapEntry {
                sorting_key: SortingKeyWrapper(i),
                filtering_key: i * 2,
                value: ValueWrapper(i * 2),
            };

            treap.set(entry);
        }

        assert_eq!(SORTING_KEY_DROP_COUNTER.get(), 10);
        assert_eq!(VALUE_DROP_COUNTER.get(), 10);

        drop(treap);

        assert_eq!(SORTING_KEY_DROP_COUNTER.get(), 20);
        assert_eq!(VALUE_DROP_COUNTER.get(), 20);
    }
}
