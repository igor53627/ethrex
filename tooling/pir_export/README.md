# ethrex-pir-export

Export UBT (Unified Binary Trie) state snapshots for PIR (Private Information Retrieval) database generation.

## Overview

This tool exports Ethereum state from ethrex in a fixed-size binary format suitable for building PIR databases. It's designed to work as a sidecar to ethrex, enabling integration with [inspire-exex](https://github.com/igor53627/inspire-exex) for private Ethereum state queries.

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
- `--hashed`: Use hashed keys mode (96-byte records) when preimages unavailable

## Output Formats

### Plain Keys Mode (default, 84 bytes per record)

```
[address: 20 bytes][slot: 32 bytes][value: 32 bytes]
```

This mode requires the node to have been synced with UBT tracking enabled, which populates the `PLAIN_STORAGE` table with original (unhashed) keys.

### Hashed Keys Mode (96 bytes per record)

```
[hashed_address: 32 bytes][hashed_slot: 32 bytes][value: 32 bytes]
```

Fallback mode that works with any ethrex node but uses keccak-hashed keys.

## Integration with inspire-exex

Data flow:

```
ethrex (synced with UBT)
    --> ethrex-pir-export
    --> state.bin
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

## Related

- [inspire-exex](https://github.com/igor53627/inspire-exex) - PIR system for private Ethereum queries
- [EIP-7864](https://eips.ethereum.org/EIPS/eip-7864) - Ethereum State Using a Unified Binary Trie
