//! Child trie host interface for Substrate runtime.
#![allow(dead_code)]

use sp_runtime_interface::runtime_interface;

/// Interface for accessing the storage from within the runtime.
#[runtime_interface]
pub trait Storage {
    /// All Child api uses :
    /// - A `child_storage_key` to define the anchor point for the child proof
    /// (commonly the location where the child root is stored in its parent trie).
    /// - A `child_storage_types` to identify the kind of the child type and how its
    /// `child definition` parameter is encoded.
    /// - A `child_definition_parameter` which is the additional information required
    /// to use the child trie. For instance defaults child tries requires this to
    /// contain a collision free unique id.
    ///
    /// This function specifically returns the data for `key` in the child storage or `None`
    /// if the key can not be found.
    fn child_get(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
    ) -> Option<Vec<u8>> {
        sp_io::default_child_storage::get(storage_key, key)
    }

    /// Get `key` from child storage, placing the value into `value_out` and return the number
    /// of bytes that the entry in storage has beyond the offset or `None` if the storage entry
    /// doesn't exist at all.
    /// If `value_out` length is smaller than the returned length, only `value_out` length bytes
    /// are copied into `value_out`.
    ///
    /// See `child_get` for common child api parameters.
    fn child_read(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
        value_out: &mut [u8],
        value_offset: u32,
    ) -> Option<u32> {
        sp_io::default_child_storage::read(storage_key, key, value_out, value_offset)
    }


    /// Set `key` to `value` in the child storage denoted by `storage_key`.
    ///
    /// See `child_get` for common child api parameters.
    fn child_set(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
        value: &[u8],
    ) {
        sp_io::default_child_storage::set(storage_key, key, value);
    }

    /// Clear the given child storage of the given `key` and its value.
    ///
    /// See `child_get` for common child api parameters.
    fn child_clear(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
    ) {
        sp_io::default_child_storage::clear(storage_key, key);
    }

    /// Clear an entire child storage.
    ///
    /// See `child_get` for common child api parameters.
    fn child_storage_kill(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
    ) {
        sp_io::default_child_storage::storage_kill(storage_key);
    }


    /// Check whether the given `key` exists in storage.
    ///
    /// See `child_get` for common child api parameters.
    fn child_exists(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
    ) -> bool {
        sp_io::default_child_storage::exists(storage_key, key)
    }

    /// Clear the child storage of each key-value pair where the key starts with the given `prefix`.
    ///
    /// See `child_get` for common child api parameters.
    fn child_clear_prefix(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        prefix: &[u8],
    ) {
        sp_io::default_child_storage::clear_prefix(storage_key, prefix);
    }

    /// "Commit" all existing operations and compute the resulting child storage root.
    ///
    /// The hashing algorithm is defined by the `Block`.
    ///
    /// Returns the SCALE encoded hash.
    ///
    /// See `child_get` for common child api parameters.
    fn child_root(
        storage_key: &[u8],
    ) -> Vec<u8> {
        sp_io::default_child_storage::root(storage_key)
    }

    /// Get the next key in storage after the given one in lexicographic order in child storage.
    fn child_next_key(
        storage_key: &[u8],
        _child_definition: &[u8],
        _child_type: u32,
        key: &[u8],
    ) -> Option<Vec<u8>> {
        sp_io::default_child_storage::next_key(storage_key, key)
    }
}
