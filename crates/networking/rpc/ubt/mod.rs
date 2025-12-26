use crate::utils::RpcErr;
use crate::{RpcApiContext, RpcHandler};
use ethrex_common::types::BlockNumber;
use serde_json::Value;

pub struct GetRootRequest {
    block_number: BlockNumber,
}

impl RpcHandler for GetRootRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = params
            .as_ref()
            .ok_or(RpcErr::MissingParam("params".to_string()))?;

        let block_number = params
            .first()
            .ok_or(RpcErr::MissingParam("block_number".to_string()))?
            .as_u64()
            .ok_or(RpcErr::BadParams(
                "block_number must be a number".to_string(),
            ))?;

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
                return Err(RpcErr::BadParams(format!(
                    "UBT only has root for current head block {}. Requested block {}.",
                    current_head.unwrap_or(0),
                    self.block_number
                )));
            }

            let root = state.root();
            Ok(serde_json::to_value(format!("{root:#x}"))?)
        }

        #[cfg(not(feature = "ubt"))]
        {
            let _ = (self, context);
            Err(RpcErr::Internal(
                "UBT feature not enabled. Rebuild with --features ubt".to_string(),
            ))
        }
    }
}
