use crate::utils::RpcErr;
use crate::{RpcApiContext, RpcHandler};
use ethrex_common::types::BlockNumber;
use serde_json::Value;

/// RPC request for `ubt_getRoot`.
///
/// Returns the UBT (Unified Binary Tree) root hash for the specified block.
/// Currently only supports querying the current head block, as historical
/// roots are not stored.
///
/// # Parameters
/// - `block_number`: The block number to query (u64 or hex string like "0x10")
///
/// # Returns
/// - The 32-byte UBT root hash as a hex string (e.g., "0x1234...")
///
/// # Errors
/// - `BadParams` if requested block != current head
/// - `Internal` if UBT is not initialized
/// - `UnsuportedFork` if UBT feature is not enabled
pub struct GetRootRequest {
    block_number: BlockNumber,
}

/// Parse a block number from JSON value, accepting both numeric and hex string formats.
fn parse_block_number(value: &Value) -> Result<u64, RpcErr> {
    if let Some(n) = value.as_u64() {
        Ok(n)
    } else if let Some(s) = value.as_str() {
        u64::from_str_radix(s.trim_start_matches("0x"), 16)
            .map_err(|_| RpcErr::BadParams("Invalid block number format".to_string()))
    } else {
        Err(RpcErr::BadParams(
            "block_number must be a number or hex string".to_string(),
        ))
    }
}

impl RpcHandler for GetRootRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = params
            .as_ref()
            .ok_or(RpcErr::MissingParam("params".to_string()))?;

        let block_number_val = params
            .first()
            .ok_or(RpcErr::MissingParam("block_number".to_string()))?;

        let block_number = parse_block_number(block_number_val)?;

        Ok(Self { block_number })
    }

    async fn handle(&self, context: RpcApiContext) -> Result<Value, RpcErr> {
        #[cfg(feature = "ubt")]
        {
            let ubt_state = context.storage.ubt_state();
            let mut state = ubt_state
                .lock()
                .map_err(|e| RpcErr::Internal(format!("Failed to lock UBT state: {e}")))?;

            let current_head = state.current_head();
            if current_head.is_none() {
                return Err(RpcErr::Internal("UBT not initialized".to_string()));
            }

            if Some(self.block_number) != current_head {
                let head = current_head.expect("checked is_none above");
                return Err(RpcErr::BadParams(format!(
                    "UBT only has root for current head block {head}. Requested block {}.",
                    self.block_number
                )));
            }

            let root = state.root();
            Ok(serde_json::to_value(format!("{root:#x}"))?)
        }

        #[cfg(not(feature = "ubt"))]
        {
            let _ = (self, context);
            Err(RpcErr::UnsuportedFork(
                "UBT feature not enabled. Rebuild with --features ubt".to_string(),
            ))
        }
    }
}
