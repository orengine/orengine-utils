//! Provides utilities for working with NUMA (Non-Uniform Memory Access) nodes.
//!
//! This module offers functionality to:
//! - Manage data per NUMA node with [`DataPerNUMANodeManager`]
//! - Get information about available NUMA nodes
//! - Control thread affinity to specific NUMA nodes
//!
//! # Example
//!
//! ```
//! use orengine_utils::numa::{DataPerNUMANodeManager, get_current_thread_numa_node};
//!
//! // Create a manager with data for each NUMA node
//! static MANAGER: DataPerNUMANodeManager<usize> = DataPerNUMANodeManager::from_arr([0; 64]);
//!
//! let numa_node = get_current_thread_numa_node();
//! println!("Memory by {numa_node} NUMA node contains {}", MANAGER.get_ref_by_node(numa_node));
//! ```

use crate::hints::unwrap_or_bug_message_hint;
use core::iter::Iterator;
use std::mem::MaybeUninit;

#[cfg(not(feature = "more_numa_nodes"))]
pub const MAX_NUMA_NODES_SUPPORTED_: usize = 64;
#[cfg(feature = "more_numa_nodes")]
pub const MAX_NUMA_NODES_SUPPORTED_: usize = 1024;

/// The maximum number of NUMA nodes supported by the library.
///
/// If a machine supports more NUMA nodes than this, panics may occur with `debug_assertions`
/// or UB otherwise.
pub const MAX_NUMA_NODES_SUPPORTED: usize = MAX_NUMA_NODES_SUPPORTED_;

const NUMA_NODE_TOO_LARGE: &'static str = "this hardware supports more NUMA-nodes than expected, use the `more_numa_nodes` feature to increase the limit";

/// Manages data per NUMA node.
/// It allows storing data for each NUMA node and accessing it by the NUMA node ID.
///
/// # Example
///
/// ```rust
/// use orengine_utils::numa::{DataPerNUMANodeManager, get_current_thread_numa_node};
///
/// // Create a manager with data for each NUMA node
/// static MANAGER: DataPerNUMANodeManager<usize> = DataPerNUMANodeManager::from_arr([0; 64]);
///
/// let numa_node = get_current_thread_numa_node();
/// println!("Memory by {numa_node} NUMA node contains {}", MANAGER.get_ref_by_node(numa_node));
/// ```
pub struct DataPerNUMANodeManager<T>([T; MAX_NUMA_NODES_SUPPORTED]);

impl<T> DataPerNUMANodeManager<T> {
    /// Creates a new manager from an array of data for each NUMA node.
    pub const fn from_arr(inner: [T; MAX_NUMA_NODES_SUPPORTED]) -> Self {
        Self(inner)
    }

    /// Gets a reference to the data for the specified NUMA node.
    ///
    /// # Panics
    ///
    /// Panics if the NUMA node ID is out of bounds.
    pub fn get_ref_by_node(&self, numa_node: usize) -> &T {
        unwrap_or_bug_message_hint(self.0.get(numa_node), NUMA_NODE_TOO_LARGE)
    }

    /// Gets a mutable reference to the data for the specified NUMA node.
    ///
    /// # Panics
    ///
    /// Panics if the NUMA node ID is out of bounds.
    pub fn get_mut_by_node(&mut self, numa_node: usize) -> &mut T {
        unwrap_or_bug_message_hint(self.0.get_mut(numa_node), NUMA_NODE_TOO_LARGE)
    }

    /// Returns an iterator over references to the data for all NUMA nodes.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Returns an iterator over mutable references to the data for all NUMA nodes.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }

    /// Returns a pointer to the inner array.
    pub fn as_ptr(&self) -> *const [T; MAX_NUMA_NODES_SUPPORTED] {
        self.0.as_ptr().cast()
    }
}

impl<T: Default> Default for DataPerNUMANodeManager<T> {
    fn default() -> Self {
        Self(core::array::from_fn(|_| T::default()))
    }
}

/// Gets the NUMA node ID for the current thread.
///
/// Returns the NUMA node that the current thread is running on.
/// If NUMA is not supported, returns 0.
///
/// # Examples
///
/// ```
/// use orengine_utils::numa::get_current_thread_numa_node;
///
/// let node_id = get_current_thread_numa_node();
/// println!("Current thread is on NUMA node {}", node_id);
/// ```
pub fn get_current_thread_numa_node() -> usize {
    #[cfg(target_os = "linux")]
    {
        let mut numa_node: MaybeUninit<u32> = MaybeUninit::uninit();

        unsafe {
            libc::syscall(
                libc::SYS_getcpu,
                core::ptr::null::<libc::c_void>(),
                numa_node.as_mut_ptr(),
                core::ptr::null::<libc::c_void>(),
            );
        }

        unsafe { numa_node.assume_init() as usize }
    }

    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_per_numa_node_manager_iterators() {
        let mut arr = [1i32; MAX_NUMA_NODES_SUPPORTED];
        for i in 0..8 {
            arr[i] = (i + 1) as i32;
        }
        let mut manager = DataPerNUMANodeManager::from_arr(arr);

        // Test iter()
        let values: Vec<i32> = manager.iter().cloned().collect();
        assert_eq!(values[0], 1);
        assert_eq!(values[7], 8);
        assert_eq!(values[8], 1); // rest should be default (1)

        // Test iter_mut()
        for val in manager.iter_mut().take(4) {
            *val *= 2;
        }
        assert_eq!(*manager.get_ref_by_node(0), 2);
        assert_eq!(*manager.get_ref_by_node(3), 8);
        assert_eq!(*manager.get_ref_by_node(4), 5);

        // Test iter_enumerated()
        let enumerated: Vec<(usize, &i32)> = manager.iter().enumerate().collect();
        assert_eq!(enumerated[0], (0, &2));
        assert_eq!(enumerated[3], (3, &8));
        assert_eq!(enumerated[4], (4, &5));

        // Test iter_enumerated_mut()
        for (node_id, val) in manager.iter_mut().enumerate() {
            if node_id % 2 == 0 {
                *val += 10;
            }
        }
        assert_eq!(*manager.get_ref_by_node(0), 12);
        assert_eq!(*manager.get_ref_by_node(1), 4);
        assert_eq!(*manager.get_ref_by_node(2), 16);
    }

    #[test]
    fn test_get_current_thread_numa_node() {
        let node_id = get_current_thread_numa_node();
        println!("Current thread is on NUMA node {}", node_id);
    }

    #[test]
    fn test_data_per_numa_node_manager_bounds() {
        let manager = DataPerNUMANodeManager::from_arr([0u8; MAX_NUMA_NODES_SUPPORTED]);

        // Should work for valid indices
        for i in 0..MAX_NUMA_NODES_SUPPORTED {
            let _ref = manager.get_ref_by_node(i);
        }
    }

    #[test]
    fn test_common_case() {
        let numa_node = get_current_thread_numa_node();
        let manager = DataPerNUMANodeManager::from_arr([0u8; MAX_NUMA_NODES_SUPPORTED]);

        assert_eq!(*manager.get_ref_by_node(numa_node), 0);
    }
}
