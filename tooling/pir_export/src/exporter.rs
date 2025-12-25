use ethrex_common::H256;
use ethrex_storage::Store;
use eyre::Result;
use std::io::Write;
use tracing::debug;

/// Export storage using hashed keys (fallback when preimages unavailable).
/// Record format: [hashed_address: 32][hashed_slot: 32][value: 32] = 96 bytes
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

/// Export storage using plain (unhashed) keys.
/// Record format: [address: 20][slot: 32][value: 32] = 84 bytes
///
/// This requires the PLAIN_STORAGE table to be populated with preimages.
pub async fn export_plain<W: Write>(
    store: &Store,
    state_root: H256,
    writer: &mut W,
) -> Result<u64> {
    let mut count = 0u64;
    let mut record = [0u8; 84];

    let iter = store.iter_plain_storage(state_root).await?;

    for (address, slot, value) in iter {
        if value.is_zero() {
            continue;
        }

        record[0..20].copy_from_slice(address.as_bytes());
        record[20..52].copy_from_slice(slot.as_bytes());
        record[52..84].copy_from_slice(&value.to_big_endian());

        writer.write_all(&record)?;
        count += 1;

        if count.is_multiple_of(1_000_000) {
            debug!("Exported {} storage entries", count);
        }
    }

    Ok(count)
}
