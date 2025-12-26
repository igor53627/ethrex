use ethrex_common::H256;
use ethrex_common::utils::keccak;
use ethrex_storage::Store;
use eyre::Result;
use std::io::Write;
use tracing::{debug, info};

pub const STATE_MAGIC: [u8; 4] = *b"PIR2";
pub const STATE_VERSION: u16 = 1;
pub const STATE_HEADER_SIZE: usize = 64;
pub const STATE_ENTRY_SIZE_PLAIN: usize = 84;
const _STATE_ENTRY_SIZE_HASHED: usize = 96;

/// State file header (64 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct StateHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub entry_size: u16,
    pub entry_count: u64,
    pub block_number: u64,
    pub chain_id: u64,
    pub block_hash: [u8; 32],
}

impl StateHeader {
    pub fn new(
        entry_size: u16,
        entry_count: u64,
        block_number: u64,
        chain_id: u64,
        block_hash: H256,
    ) -> Self {
        Self {
            magic: STATE_MAGIC,
            version: STATE_VERSION,
            entry_size,
            entry_count,
            block_number,
            chain_id,
            block_hash: block_hash.0,
        }
    }

    pub fn as_bytes(self) -> [u8; STATE_HEADER_SIZE] {
        let mut buf = [0u8; STATE_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4..6].copy_from_slice(&self.version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.entry_size.to_le_bytes());
        buf[8..16].copy_from_slice(&self.entry_count.to_le_bytes());
        buf[16..24].copy_from_slice(&self.block_number.to_le_bytes());
        buf[24..32].copy_from_slice(&self.chain_id.to_le_bytes());
        buf[32..64].copy_from_slice(&self.block_hash);
        buf
    }
}

/// Storage entry for sorting
struct SortableEntry {
    sort_key: [u8; 32],
    data: [u8; 84],
}

/// Export storage using plain (unhashed) keys with PIR2 header.
/// Record format: [address: 20][slot: 32][value: 32] = 84 bytes
///
/// Entries are sorted by keccak256(address || slot) for bucket index compatibility.
pub fn export_plain<W: Write>(
    store: &Store,
    block_number: u64,
    chain_id: u64,
    block_hash: H256,
    writer: &mut W,
) -> Result<u64> {
    info!("Collecting storage entries...");
    let mut entries: Vec<SortableEntry> = Vec::new();

    store.iter_plain_storage(|address, slot, value| {
        if value.is_zero() {
            return Ok(());
        }

        let mut concat = [0u8; 52];
        concat[0..20].copy_from_slice(address.as_bytes());
        concat[20..52].copy_from_slice(slot.as_bytes());
        let sort_key = keccak(concat).0;

        let mut data = [0u8; 84];
        data[0..20].copy_from_slice(address.as_bytes());
        data[20..52].copy_from_slice(slot.as_bytes());
        data[52..84].copy_from_slice(&value.to_big_endian());

        entries.push(SortableEntry { sort_key, data });

        if entries.len().is_multiple_of(1_000_000) {
            debug!("Collected {} storage entries", entries.len());
        }

        Ok::<(), std::io::Error>(())
    })?;

    let count = entries.len() as u64;
    info!(
        "Collected {} entries, sorting by keccak256(address || slot)...",
        count
    );

    entries.sort_unstable_by(|a, b| a.sort_key.cmp(&b.sort_key));

    info!("Writing header and {} entries...", count);

    let header = StateHeader::new(
        STATE_ENTRY_SIZE_PLAIN as u16,
        count,
        block_number,
        chain_id,
        block_hash,
    );
    writer.write_all(&header.as_bytes())?;

    for (i, entry) in entries.iter().enumerate() {
        writer.write_all(&entry.data)?;
        if (i + 1).is_multiple_of(1_000_000) {
            debug!("Written {} entries", i + 1);
        }
    }

    Ok(count)
}

/// Export storage using hashed keys (fallback when preimages unavailable).
/// Record format: [hashed_address: 32][hashed_slot: 32][value: 32] = 96 bytes
///
/// Note: Hashed mode does NOT use the PIR2 header format since it uses
/// different entry size (96 bytes) and is only a fallback for legacy exports.
pub fn export_hashed<W: Write>(store: &Store, state_root: H256, writer: &mut W) -> Result<u64> {
    let mut count = 0u64;
    let mut record = [0u8; 96];

    let accounts_iter = store.iter_accounts(state_root)?;

    for (hashed_address, account_state) in accounts_iter {
        if account_state.storage_root == *ethrex_common::constants::EMPTY_TRIE_HASH {
            continue;
        }

        let storage_iter = match store.iter_storage(state_root, hashed_address)? {
            Some(iter) => iter,
            None => continue,
        };

        for (hashed_slot, value) in storage_iter {
            if value.is_zero() {
                continue;
            }

            record[0..32].copy_from_slice(hashed_address.as_bytes());
            record[32..64].copy_from_slice(hashed_slot.as_bytes());
            record[64..96].copy_from_slice(&value.to_big_endian());

            writer.write_all(&record)?;
            count += 1;

            if count.is_multiple_of(1_000_000) {
                debug!("Exported {} storage entries", count);
            }
        }
    }

    Ok(count)
}
