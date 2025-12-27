use crate::utils::RpcErr;
use crate::{RpcApiContext, RpcHandler};
use ethrex_common::{Address, H256, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_DELTA_BLOCKS: u64 = 100;
const MAX_DUMP_ENTRIES: usize = 10_000;

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StorageDelta {
    pub address: Address,
    pub slot: H256,
    pub value: U256,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct GetStateDeltaResponse {
    pub from_block: u64,
    pub to_block: u64,
    pub deltas: Vec<StorageDelta>,
}

#[allow(dead_code)]
pub struct GetStateDeltaRequest {
    from_block: u64,
    to_block: u64,
}

impl RpcHandler for GetStateDeltaRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = params
            .as_ref()
            .ok_or(RpcErr::MissingParam("params".to_string()))?;

        let from_block = params
            .first()
            .ok_or(RpcErr::MissingParam("from_block".to_string()))?
            .as_u64()
            .ok_or(RpcErr::BadParams("from_block must be a number".to_string()))?;

        let to_block = params
            .get(1)
            .ok_or(RpcErr::MissingParam("to_block".to_string()))?
            .as_u64()
            .ok_or(RpcErr::BadParams("to_block must be a number".to_string()))?;

        if to_block < from_block {
            return Err(RpcErr::BadParams(
                "to_block must be >= from_block".to_string(),
            ));
        }

        let block_count = to_block - from_block + 1;
        if block_count > MAX_DELTA_BLOCKS {
            return Err(RpcErr::BadParams(format!(
                "Block count {block_count} exceeds maximum of {MAX_DELTA_BLOCKS} blocks"
            )));
        }

        Ok(Self {
            from_block,
            to_block,
        })
    }

    async fn handle(&self, _context: RpcApiContext) -> Result<Value, RpcErr> {
        Err(RpcErr::MethodNotFound(
            "pir_getStateDelta: not yet implemented, requires historical state delta tracking"
                .to_string(),
        ))
    }
}

#[derive(Debug, Serialize)]
pub struct DumpStorageEntry {
    pub address: Address,
    pub slot: H256,
    pub value: U256,
}

#[derive(Debug, Serialize)]
pub struct DumpStorageResponse {
    pub entries: Vec<DumpStorageEntry>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

pub struct DumpStorageRequest {
    cursor: Option<(Address, H256)>,
    limit: usize,
}

impl RpcHandler for DumpStorageRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = params.as_ref();

        let cursor = if let Some(params) = params {
            if let Some(cursor_val) = params.first() {
                if cursor_val.is_null() {
                    None
                } else {
                    let cursor_str = cursor_val
                        .as_str()
                        .ok_or(RpcErr::BadParams("cursor must be a string".to_string()))?;

                    if cursor_str.is_empty() {
                        None
                    } else {
                        let bytes =
                            hex::decode(cursor_str.trim_start_matches("0x")).map_err(|_| {
                                RpcErr::BadParams("Invalid cursor hex format".to_string())
                            })?;
                        if bytes.len() != 52 {
                            return Err(RpcErr::BadParams(
                                "Cursor must be 52 bytes (20 address + 32 slot)".to_string(),
                            ));
                        }
                        let address = Address::from_slice(&bytes[0..20]);
                        let slot = H256::from_slice(&bytes[20..52]);
                        Some((address, slot))
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let limit = if let Some(params) = params {
            match params.get(1) {
                None | Some(Value::Null) => 1000,
                Some(v) => v
                    .as_u64()
                    .ok_or_else(|| RpcErr::BadParams("limit must be a number".to_string()))?
                    as usize,
            }
        } else {
            1000
        };

        if limit > MAX_DUMP_ENTRIES {
            return Err(RpcErr::BadParams(format!(
                "limit exceeds maximum of {MAX_DUMP_ENTRIES}"
            )));
        }

        if limit == 0 {
            return Err(RpcErr::BadParams("limit must be > 0".to_string()));
        }

        Ok(Self { cursor, limit })
    }

    async fn handle(&self, context: RpcApiContext) -> Result<Value, RpcErr> {
        let mut entries = Vec::with_capacity(self.limit + 1);
        let cursor_ref = &self.cursor;
        let limit = self.limit;
        let mut done = false;

        context
            .storage
            .iter_plain_storage(|address, slot, value| {
                if done {
                    return Ok::<(), std::io::Error>(());
                }

                if value.is_zero() {
                    return Ok(());
                }

                if let Some((cursor_addr, cursor_slot)) = cursor_ref
                    && (address, slot) <= (*cursor_addr, *cursor_slot)
                {
                    return Ok(());
                }

                entries.push(DumpStorageEntry {
                    address,
                    slot,
                    value,
                });

                if entries.len() > limit {
                    done = true;
                }

                Ok(())
            })
            .map_err(|e| RpcErr::Internal(format!("Failed to iterate storage: {e}")))?;

        let has_more = entries.len() > self.limit;
        if has_more {
            entries.pop();
        }

        let next_cursor = if has_more {
            entries.last().map(|entry| {
                let mut cursor_bytes = [0u8; 52];
                cursor_bytes[0..20].copy_from_slice(entry.address.as_bytes());
                cursor_bytes[20..52].copy_from_slice(entry.slot.as_bytes());
                format!("0x{}", hex::encode(cursor_bytes))
            })
        } else {
            None
        };

        let response = DumpStorageResponse {
            entries,
            next_cursor,
            has_more,
        };

        Ok(serde_json::to_value(response)?)
    }
}
