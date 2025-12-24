//! UBT (Unified Binary Tree) state tracking for EIP-7864.
//!
//! This module maintains an auxiliary UBT alongside the MPT state,
//! computing UBT roots for each canonical block.

use ethereum_types::H256;
use ubt::{B256, Blake3Hasher, TreeKey, UnifiedBinaryTree};

/// Block number type alias for clarity.
pub type BlockNumber = u64;

/// A single UBT update entry (key-value pair for batch insert).
#[derive(Debug, Clone)]
pub struct UbtUpdate {
    /// The 32-byte tree key (31-byte stem + 1-byte subindex).
    pub key: TreeKey,
    /// The value to insert, or None for deletion.
    pub value: Option<B256>,
}

/// UBT state tracking.
///
/// Maintains an in-memory UBT that is updated on each canonical block.
/// The UBT is computed alongside the MPT but is not consensus-critical.
pub struct UbtState {
    /// The UBT tree (in-memory).
    tree: UnifiedBinaryTree<Blake3Hasher>,
    /// Current block number the UBT is synced to.
    current_head: Option<BlockNumber>,
    /// Whether the UBT is currently being rebuilt (e.g., after reorg).
    rebuilding: bool,
}

impl std::fmt::Debug for UbtState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UbtState")
            .field("current_head", &self.current_head)
            .field("rebuilding", &self.rebuilding)
            .field("stem_count", &self.tree.len())
            .finish()
    }
}

impl Default for UbtState {
    fn default() -> Self {
        Self::new()
    }
}

impl UbtState {
    /// Create a new empty UBT state.
    pub fn new() -> Self {
        Self {
            tree: UnifiedBinaryTree::new(),
            current_head: None,
            rebuilding: false,
        }
    }

    /// Create a new UBT state with pre-allocated capacity.
    pub fn with_capacity(stem_capacity: usize) -> Self {
        Self {
            tree: UnifiedBinaryTree::with_capacity(stem_capacity),
            current_head: None,
            rebuilding: false,
        }
    }

    /// Get the current UBT root hash.
    pub fn root(&mut self) -> H256 {
        let b256_root = self.tree.root_hash();
        H256::from_slice(b256_root.as_slice())
    }

    /// Get the current block number the UBT is synced to.
    pub fn current_head(&self) -> Option<BlockNumber> {
        self.current_head
    }

    /// Check if the UBT is currently rebuilding.
    pub fn is_rebuilding(&self) -> bool {
        self.rebuilding
    }

    /// Set the rebuilding flag.
    pub fn set_rebuilding(&mut self, rebuilding: bool) {
        self.rebuilding = rebuilding;
    }

    /// Apply a batch of updates for a single block.
    ///
    /// Returns the new UBT root hash after applying the updates.
    pub fn apply_block_updates(
        &mut self,
        block_number: BlockNumber,
        _block_hash: H256,
        updates: &[UbtUpdate],
    ) -> H256 {
        let entries: Vec<(TreeKey, B256)> = updates
            .iter()
            .filter_map(|u| u.value.map(|v| (u.key, v)))
            .collect();

        if !entries.is_empty() {
            self.tree.insert_batch(entries);
        }

        self.current_head = Some(block_number);

        self.root()
    }

    /// Reset the UBT state (e.g., for reorg handling).
    pub fn reset(&mut self) {
        self.tree = UnifiedBinaryTree::new();
        self.current_head = None;
        self.rebuilding = true;
    }

    /// Get the number of stems in the tree (for diagnostics).
    pub fn stem_count(&self) -> usize {
        self.tree.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ubt::{Address, BasicDataLeaf, get_basic_data_key, get_storage_slot_key};

    #[test]
    fn test_empty_tree_root() {
        let mut state = UbtState::new();
        let root = state.root();
        assert_eq!(root, H256::zero());
    }

    #[test]
    fn test_single_insert() {
        let mut state = UbtState::new();

        let address = Address::repeat_byte(0x42);
        let key = get_basic_data_key(&address);
        let leaf = BasicDataLeaf::new(1, 1000, 0);

        let updates = vec![UbtUpdate {
            key,
            value: Some(leaf.encode()),
        }];

        let root = state.apply_block_updates(1, H256::zero(), &updates);
        assert_ne!(root, H256::zero());
        assert_eq!(state.current_head(), Some(1));
    }

    #[test]
    fn test_storage_slot_insert() {
        let mut state = UbtState::new();

        let address = Address::repeat_byte(0x42);
        let slot = [0u8; 32];
        let key = get_storage_slot_key(&address, &slot);
        let value = B256::repeat_byte(0xff);

        let updates = vec![UbtUpdate {
            key,
            value: Some(value),
        }];

        let root = state.apply_block_updates(1, H256::zero(), &updates);
        assert_ne!(root, H256::zero());
    }

    #[test]
    fn test_reset() {
        let mut state = UbtState::new();

        let address = Address::repeat_byte(0x42);
        let key = get_basic_data_key(&address);
        let leaf = BasicDataLeaf::new(1, 1000, 0);

        let updates = vec![UbtUpdate {
            key,
            value: Some(leaf.encode()),
        }];

        state.apply_block_updates(1, H256::zero(), &updates);
        assert_ne!(state.root(), H256::zero());

        state.reset();
        assert_eq!(state.root(), H256::zero());
        assert_eq!(state.current_head(), None);
        assert!(state.is_rebuilding());
    }
}
