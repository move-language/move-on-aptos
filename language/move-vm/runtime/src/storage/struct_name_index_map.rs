// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::errors::PartialVMResult;
use move_core_types::language_storage::{StructTag, TypeTag};
use move_vm_types::{
    loaded_data::runtime_types::{StructIdentifier, StructNameIndex},
    panic_error,
};
use parking_lot::RwLock;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
struct IndexMap<T: Clone + Ord> {
    forward_map: BTreeMap<T, usize>,
    backward_map: Vec<Arc<T>>,
}

/// A data structure to cache struct identifiers (address, module name, struct name) and use
/// indices instead, to save on the memory consumption and avoid unnecessary cloning. It
/// guarantees that the same struct name identifier always corresponds to a unique index.
pub(crate) struct StructNameIndexMap(RwLock<IndexMap<StructIdentifier>>);

impl StructNameIndexMap {
    /// Returns an empty map with no entries.
    pub(crate) fn empty() -> Self {
        Self(RwLock::new(IndexMap {
            forward_map: BTreeMap::new(),
            backward_map: vec![],
        }))
    }

    /// Flushes the cached struct names and indices.
    pub(crate) fn flush(&self) {
        let mut index_map = self.0.write();
        index_map.backward_map.clear();
        index_map.forward_map.clear();
    }

    /// Maps the struct identifier into an index. If the identifier already exists returns the
    /// corresponding index. This function guarantees that for any struct identifiers A and B,
    /// if A == B, they have the same indices.
    pub(crate) fn struct_name_to_idx(
        &self,
        struct_name: StructIdentifier,
    ) -> PartialVMResult<StructNameIndex> {
        // Note that we take a write lock here once, instead of (*): taking a read lock, checking
        // if the index is cached, re-acquiring the (write) lock, and checking again, as it makes
        // things faster.
        // Note that even if we do (*), we MUST check if another thread has cached the index before
        // we reached this point for correctness. If we do not do this, we can end up evicting the
        // previous index, and end up with multiple indices corresponding to the same struct. As
        // indices are stored inside types, type comparison breaks!
        let mut index_map = self.0.write();

        // Index is cached, return early.
        if let Some(idx) = index_map.forward_map.get(&struct_name) {
            return Ok(StructNameIndex(*idx));
        }

        // Otherwise, the cache is locked and the struct name is not present. We simply add it
        // to the cache and return the corresponding index.
        let idx = index_map.backward_map.len();
        let prev_idx = index_map.forward_map.insert(struct_name.clone(), idx);
        index_map.backward_map.push(Arc::new(struct_name));

        // Unlock the cache.
        drop(index_map);

        if prev_idx.is_some() {
            return Err(panic_error!(
                "Indexing map should never evict cached entries"
            ));
        }
        Ok(StructNameIndex(idx))
    }

    /// Returns the reference of the struct name corresponding to the index. Here, we wrap the
    /// name into an [Arc] to ensure that the lock is released.
    pub(crate) fn idx_to_struct_name_ref(
        &self,
        idx: StructNameIndex,
    ) -> PartialVMResult<Arc<StructIdentifier>> {
        let index_map = self.0.read();
        Ok(index_map
            .backward_map
            .get(idx.0)
            .ok_or_else(|| {
                let msg = format!(
                    "Index out of bounds when accessing struct name reference \
                     at index {}, backward map length: {}",
                    idx.0,
                    index_map.backward_map.len()
                );
                panic_error!(msg)
            })?
            .clone())
    }

    /// Returns the clone of the struct name corresponding to the index. The clone ensures that the
    /// lock is released before the control returns to the caller.
    pub(crate) fn idx_to_struct_name(
        &self,
        idx: StructNameIndex,
    ) -> PartialVMResult<StructIdentifier> {
        let index_map = self.0.read();
        Ok(index_map
            .backward_map
            .get(idx.0)
            .ok_or_else(|| {
                let msg = format!(
                    "Index out of bounds when accessing struct name at index {}, \
                     backward map length: {}",
                    idx.0,
                    index_map.backward_map.len()
                );
                panic_error!(msg)
            })?
            .as_ref()
            .clone())
    }

    /// Returns the struct tag corresponding to the struct name and the provided type arguments.
    pub(crate) fn idx_to_struct_tag(
        &self,
        idx: StructNameIndex,
        ty_args: Vec<TypeTag>,
    ) -> PartialVMResult<StructTag> {
        let index_map = self.0.read();
        let struct_name = index_map
            .backward_map
            .get(idx.0)
            .ok_or_else(|| {
                let msg = format!(
                    "Index out of bounds when constructing a struct tag \
                     for struct name at index {}, backward map length: {}",
                    idx.0,
                    index_map.backward_map.len()
                );
                panic_error!(msg)
            })?
            .as_ref();
        Ok(StructTag {
            address: *struct_name.module.address(),
            module: struct_name.module.name().to_owned(),
            name: struct_name.name.clone(),
            type_args: ty_args,
        })
    }

    /// Returns the number of cached entries. Asserts that the number of cached indices is equal to
    /// the number of cached struct names.
    pub(crate) fn checked_len(&self) -> PartialVMResult<usize> {
        let index_map = self.0.read();
        let forward_map_len = index_map.forward_map.len();
        let backward_map_len = index_map.backward_map.len();
        drop(index_map);

        if forward_map_len != backward_map_len {
            let msg = format!(
                "Indexed map maps size mismatch: forward map has length {}, \
                 but backward map has length {}",
                forward_map_len, backward_map_len
            );
            return Err(panic_error!(msg));
        }

        Ok(forward_map_len)
    }
}

// Only used by V1 loader.
impl Clone for StructNameIndexMap {
    fn clone(&self) -> Self {
        Self(RwLock::new(self.0.read().clone()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use claims::{assert_err, assert_ok};
    use move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
    };
    use proptest::{arbitrary::any, collection::vec, proptest, strategy::Strategy};
    use std::sync::Arc;

    fn make_struct_name(module_name: &str, struct_name: &str) -> StructIdentifier {
        let module = ModuleId::new(AccountAddress::ONE, Identifier::new(module_name).unwrap());
        let name = Identifier::new(struct_name).unwrap();
        StructIdentifier { module, name }
    }

    #[test]
    fn test_index_map_must_contain_idx() {
        let struct_name_idx_map = StructNameIndexMap::empty();
        assert_err!(struct_name_idx_map.idx_to_struct_name_ref(StructNameIndex(0)));
    }

    #[test]
    fn test_index_map() {
        let struct_name_idx_map = StructNameIndexMap::empty();

        // First-time access.

        let foo = make_struct_name("foo", "Foo");
        let foo_idx = assert_ok!(struct_name_idx_map.struct_name_to_idx(foo.clone()));
        assert_eq!(foo_idx.0, 0);

        let bar = make_struct_name("bar", "Bar");
        let bar_idx = assert_ok!(struct_name_idx_map.struct_name_to_idx(bar.clone()));
        assert_eq!(bar_idx.0, 1);

        // Check that struct names actually correspond to indices.
        let returned_foo = assert_ok!(struct_name_idx_map.idx_to_struct_name_ref(foo_idx));
        assert_eq!(returned_foo.as_ref(), &foo);
        let returned_bar = assert_ok!(struct_name_idx_map.idx_to_struct_name_ref(bar_idx));
        assert_eq!(returned_bar.as_ref(), &bar);

        // Re-check indices on second access.
        let foo_idx = assert_ok!(struct_name_idx_map.struct_name_to_idx(foo));
        assert_eq!(foo_idx.0, 0);
        let bar_idx = assert_ok!(struct_name_idx_map.struct_name_to_idx(bar));
        assert_eq!(bar_idx.0, 1);

        let len = assert_ok!(struct_name_idx_map.checked_len());
        assert_eq!(len, 2);
    }

    fn struct_name_strategy() -> impl Strategy<Value = StructIdentifier> {
        let address = any::<AccountAddress>();
        let module_identifier = any::<Identifier>();
        let struct_identifier = any::<Identifier>();
        (address, module_identifier, struct_identifier).prop_map(|(a, m, name)| StructIdentifier {
            module: ModuleId::new(a, m),
            name,
        })
    }

    proptest! {
        #[test]
        fn test_index_map_concurrent_arbitrary_struct_names(struct_names in vec(struct_name_strategy(), 30..100),
        ) {
            let struct_name_idx_map = Arc::new(StructNameIndexMap::empty());

            // Each thread caches a struct name, and reads it to check if the cached result is
            // still the same.
            std::thread::scope(|s| {
                for struct_name in &struct_names {
                    s.spawn({
                        let struct_name_idx_map = struct_name_idx_map.clone();
                        move || {
                            let idx = assert_ok!(struct_name_idx_map.struct_name_to_idx(struct_name.clone()));
                            let actual_struct_name = assert_ok!(struct_name_idx_map.idx_to_struct_name_ref(idx));
                            assert_eq!(actual_struct_name.as_ref(), struct_name);
                        }
                    });
                }
            });
        }
    }

    #[test]
    fn test_index_map_concurrent_single_struct_name() {
        let struct_name_idx_map = Arc::new(StructNameIndexMap::empty());
        let struct_name = make_struct_name("foo", "Foo");

        // Each threads tries to cache the same struct name.
        std::thread::scope(|s| {
            for _ in 0..50 {
                s.spawn({
                    let struct_name_idx_map = struct_name_idx_map.clone();
                    let struct_name = struct_name.clone();
                    move || {
                        assert_ok!(struct_name_idx_map.struct_name_to_idx(struct_name));
                    }
                });
            }
        });

        // Only a single struct name mast be cached!
        let len = assert_ok!(struct_name_idx_map.checked_len());
        assert_eq!(len, 1);
    }
}
