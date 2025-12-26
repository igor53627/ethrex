# ethrex-pir-export

Export UBT (Unified Binary Trie) state snapshots for PIR (Private Information Retrieval) database generation.

## Overview

This tool exports Ethereum state from ethrex in the PIR2 binary format suitable for building PIR databases. It's designed to work as a sidecar to ethrex, enabling integration with [inspire-exex](https://github.com/igor53627/inspire-exex) for private Ethereum state queries.

## Usage

```bash
ethrex-pir-export \
  --datadir /path/to/ethrex/data \
  --block <block_number> \
  --output state.bin
```

### Options

- `--datadir <PATH>`: Path to ethrex data directory (required)
- `--block <N>`: Block number to export state from (defaults to latest finalized)
- `--output <PATH>`: Output file path for the state export (required)
- `--hashed`: Use hashed keys mode (legacy, 96-byte records, no header)

## Output Format (PIR2)

The output uses the STATE_FORMAT.md specification:

### File Layout

```
+------------------+
| Header (64 bytes)|
+------------------+
| Entry 0 (84 B)   |
+------------------+
| Entry 1 (84 B)   |
+------------------+
| ...              |
+------------------+
| Entry N-1 (84 B) |
+------------------+
```

### Header (64 bytes)

| Offset | Size | Field        | Description                     |
|--------|------|--------------|-------------------------------- |
| 0      | 4    | magic        | `0x50495232` ("PIR2" in ASCII)  |
| 4      | 2    | version      | Format version (1)              |
| 6      | 2    | entry_size   | Bytes per entry (84)            |
| 8      | 8    | entry_count  | Number of entries               |
| 16     | 8    | block_number | Snapshot block number           |
| 24     | 8    | chain_id     | Ethereum chain ID               |
| 32     | 32   | block_hash   | Block hash                      |

All integers are little-endian.

### Entry Format (84 bytes)

| Offset | Size | Field   | Description      |
|--------|------|---------|------------------|
| 0      | 20   | address | Contract address |
| 20     | 32   | slot    | Storage slot key |
| 52     | 32   | value   | Storage value    |

### Entry Ordering

Entries are sorted by `keccak256(address || slot)` for bucket index compatibility with the PIR database layout.

## Hashed Keys Mode (Legacy)

When `--hashed` is specified, uses 96-byte records without the PIR2 header:

```
[hashed_address: 32 bytes][hashed_slot: 32 bytes][value: 32 bytes]
```

This is a fallback mode that works with any ethrex node but uses keccak-hashed keys and does not support the inspire-setup PIR encoder.

## Integration with inspire-exex

Data flow:

```
ethrex (synced with UBT)
    --> ethrex-pir-export
    --> state.bin (PIR2 format)
    --> inspire-setup (encode PIR database)
    --> db.bin
    --> inspire-server (serve queries)
```

## Requirements

- ethrex node synced with UBT tracking enabled (for plain keys mode)
- RocksDB storage backend

## Building

```bash
cd tooling
cargo build --release -p pir_export
```

The binary will be at `target/release/ethrex-pir-export`.

## Example Output

```
2025-01-15T10:30:00 INFO ethrex_pir_export: Opening store at "/data/ethrex"
2025-01-15T10:30:01 INFO ethrex_pir_export: Using latest finalized block: 20000000
2025-01-15T10:30:01 INFO ethrex_pir_export: Exporting state at block 20000000 with state_root 0x1234...
2025-01-15T10:30:01 INFO ethrex_pir_export: Using plain keys mode (PIR2 format, 84-byte records)
2025-01-15T10:35:00 INFO exporter: Collected 150000000 entries, sorting by keccak256(address || slot)...
2025-01-15T10:36:00 INFO exporter: Writing header and 150000000 entries...
2025-01-15T10:40:00 INFO ethrex_pir_export: --- Export Summary ---
2025-01-15T10:40:00 INFO ethrex_pir_export: Format:       PIR2 v1
2025-01-15T10:40:00 INFO ethrex_pir_export: Magic:        "PIR2"
2025-01-15T10:40:00 INFO ethrex_pir_export: Header size:  64 bytes
2025-01-15T10:40:00 INFO ethrex_pir_export: Entry size:   84 bytes
2025-01-15T10:40:00 INFO ethrex_pir_export: Entry count:  150000000
2025-01-15T10:40:00 INFO ethrex_pir_export: Block number: 20000000
2025-01-15T10:40:00 INFO ethrex_pir_export: Chain ID:     1
2025-01-15T10:40:00 INFO ethrex_pir_export: Block hash:   0xabcd...
2025-01-15T10:40:00 INFO ethrex_pir_export: Total size:   12600000064 bytes (12017.36 MB)
2025-01-15T10:40:00 INFO ethrex_pir_export: Export complete: "/data/state.bin"
```

## Related

- [inspire-exex](https://github.com/igor53627/inspire-exex) - PIR system for private Ethereum queries
- [STATE_FORMAT.md](https://github.com/igor53627/inspire-exex/blob/main/docs/STATE_FORMAT.md) - Format specification
- [EIP-7864](https://eips.ethereum.org/EIPS/eip-7864) - Ethereum State Using a Unified Binary Trie
