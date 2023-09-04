use std::sync::{Arc, RwLock};

use crate::{
    fork::ForkSource,
    node::{BlockInfo, InMemoryNodeInner},
};
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use vm::{
    utils::BLOCK_GAS_LIMIT,
    vm_with_bootloader::{init_vm_inner, BlockContextMode, BootloaderJobType, TxExecutionMode},
    HistoryEnabled, OracleTools,
};
use zksync_basic_types::{H256, U64};
use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_state::StorageView;
use zksync_state::WriteStorage;
use zksync_utils::u256_to_h256;
use zksync_web3_decl::error::Web3Error;

/// Implementation of EvmNamespace
pub struct EvmNamespaceImpl<S> {
    node: Arc<RwLock<InMemoryNodeInner<S>>>,
}

impl<S> EvmNamespaceImpl<S> {
    /// Creates a new `Evm` instance with the given `node`.
    pub fn new(node: Arc<RwLock<InMemoryNodeInner<S>>>) -> Self {
        Self { node }
    }
}

#[rpc]
pub trait EvmNamespaceT {
    /// Force a single block to be mined.
    ///
    /// Mines a block independent of whether or not mining is started or stopped. Will mine an empty block if there are no available transactions to mine.
    ///
    /// # Returns
    /// The string "0x0".
    #[rpc(name = "evm_mine")]
    fn evm_mine(&self) -> BoxFuture<Result<String>>;

    /// Increase the current timestamp for the node
    ///
    /// # Parameters
    /// - `time_delta`: The number of seconds to increase time by
    ///
    /// # Returns
    /// The applied time delta to `current_timestamp` value for the InMemoryNodeInner.
    #[rpc(name = "evm_increaseTime")]
    fn increase_time(&self, time_delta_seconds: U64) -> BoxFuture<Result<U64>>;

    /// Set the current timestamp for the node
    ///
    /// # Parameters
    /// - `time_delta`: The number of seconds to increase time by
    ///
    /// # Returns
    /// The applied time delta to `current_timestamp` value for the InMemoryNodeInner.
    #[rpc(name = "evm_setTime")]
    fn set_time(&self, time: U64) -> BoxFuture<Result<i64>>;
}

impl<S: Send + Sync + 'static + ForkSource + std::fmt::Debug> EvmNamespaceT
    for EvmNamespaceImpl<S>
{
    fn evm_mine(&self) -> BoxFuture<Result<String>> {
        let inner = Arc::clone(&self.node);
        Box::pin(async move {
            match inner.write() {
                Ok(mut inner) => {
                    let tx_hash = H256::random();
                    let (keys, block, bytecodes) = {
                        let mut storage_view = StorageView::new(&inner.fork_storage);
                        let mut oracle_tools = OracleTools::new(&mut storage_view, HistoryEnabled);

                        let bootloader_code = &inner.baseline_contracts;
                        let block_context = inner.create_block_context();
                        let block_properties =
                            InMemoryNodeInner::<S>::create_block_properties(bootloader_code);
                        let block = BlockInfo {
                            batch_number: block_context.block_number,
                            block_timestamp: block_context.block_timestamp,
                            tx_hash: Some(tx_hash),
                        };

                        // init vm
                        let mut vm = init_vm_inner(
                            &mut oracle_tools,
                            BlockContextMode::NewBlock(block_context.into(), Default::default()),
                            &block_properties,
                            BLOCK_GAS_LIMIT,
                            bootloader_code,
                            TxExecutionMode::VerifyExecute,
                        );

                        vm.execute_till_block_end(BootloaderJobType::BlockPostprocessing);

                        let bytecodes = vm
                            .state
                            .decommittment_processor
                            .known_bytecodes
                            .inner()
                            .clone();

                        let modified_keys = storage_view.modified_storage_keys().clone();
                        (modified_keys, block, bytecodes)
                    };

                    for (key, value) in keys.iter() {
                        inner.fork_storage.set_value(*key, *value);
                    }

                    // Write all the factory deps.
                    for (hash, code) in bytecodes.iter() {
                        inner.fork_storage.store_factory_dep(
                            u256_to_h256(*hash),
                            code.iter()
                                .flat_map(|entry| {
                                    let mut bytes = vec![0u8; 32];
                                    entry.to_big_endian(&mut bytes);
                                    bytes.to_vec()
                                })
                                .collect(),
                        )
                    }
                    inner.blocks.insert(block.batch_number, block);
                    {
                        inner.current_timestamp += 1;
                        inner.current_batch += 1;
                        inner.current_miniblock += 1;
                    }

                    Ok("0x0".to_string())
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn increase_time(&self, time_delta_seconds: U64) -> BoxFuture<Result<U64>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            if time_delta_seconds.is_zero() {
                return Ok(time_delta_seconds);
            }

            let time_delta = time_delta_seconds.as_u64().saturating_mul(1000);
            match inner.write() {
                Ok(mut inner_guard) => {
                    inner_guard.current_timestamp =
                        inner_guard.current_timestamp.saturating_add(time_delta);
                    Ok(time_delta_seconds)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }

    fn set_time(&self, time: U64) -> BoxFuture<Result<i64>> {
        let inner = Arc::clone(&self.node);

        Box::pin(async move {
            match inner.write() {
                Ok(mut inner_guard) => {
                    let time_diff = (time.as_u64() as i128)
                        .saturating_sub(inner_guard.current_timestamp as i128)
                        as i64;
                    inner_guard.current_timestamp = time.as_u64();
                    Ok(time_diff)
                }
                Err(_) => Err(into_jsrpc_error(Web3Error::InternalError)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{http_fork_source::HttpForkSource, node::InMemoryNode};
    use zksync_core::api_server::web3::backend_jsonrpc::namespaces::eth::EthNamespaceT;

    use super::*;

    #[tokio::test]
    async fn test_evm_mine() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let start_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, true)
            .await
            .unwrap()
            .expect("block exists");

        let result = evm.evm_mine().await.expect("evm_mine");
        assert_eq!(&result, "0x0");

        let current_block = node
            .get_block_by_number(zksync_types::api::BlockNumber::Latest, true)
            .await
            .unwrap()
            .expect("block exists");
        assert_eq!(start_block.number + 1, current_block.number);
    }

    #[tokio::test]
    async fn test_increase_time_zero_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = 0u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(U64::from(increase_value_seconds))
            .await
            .expect("failed increasing timestamp")
            .as_u64();
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_increase_time_max_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = u64::MAX;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(0, timestamp_before, "initial timestamp must be non zero",);
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(U64::from(increase_value_seconds))
            .await
            .expect("failed increasing timestamp")
            .as_u64();
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            u64::MAX,
            timestamp_after,
            "timestamp did not saturate upon increase",
        );
    }

    #[tokio::test]
    async fn test_increase_time() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let increase_value_seconds = 100u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        let expected_response = increase_value_seconds;

        let actual_response = evm
            .increase_time(U64::from(increase_value_seconds))
            .await
            .expect("failed increasing timestamp")
            .as_u64();
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            increase_value_seconds.saturating_mul(1000u64),
            timestamp_after.saturating_sub(timestamp_before),
            "timestamp did not increase by the specified amount",
        );
    }

    #[tokio::test]
    async fn test_set_time_future() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 10_000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = 9000;

        let actual_response = evm
            .set_time(U64::from(new_time))
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_past() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 10u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_ne!(timestamp_before, new_time, "timestamps must be different");
        let expected_response = -990;

        let actual_response = evm
            .set_time(U64::from(new_time))
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(new_time, timestamp_after, "timestamp was not set correctly",);
    }

    #[tokio::test]
    async fn test_set_time_same_value() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        let new_time = 1000u64;
        let timestamp_before = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");
        assert_eq!(timestamp_before, new_time, "timestamps must be same");
        let expected_response = 0;

        let actual_response = evm
            .set_time(U64::from(new_time))
            .await
            .expect("failed setting timestamp");
        let timestamp_after = node
            .get_inner()
            .read()
            .map(|inner| inner.current_timestamp)
            .expect("failed reading timestamp");

        assert_eq!(expected_response, actual_response, "erroneous response");
        assert_eq!(
            timestamp_before, timestamp_after,
            "timestamp must not change",
        );
    }

    #[tokio::test]
    async fn test_set_time_edges() {
        let node = InMemoryNode::<HttpForkSource>::default();
        let evm = EvmNamespaceImpl::new(node.get_inner());

        for new_time in [0, u64::MAX] {
            let timestamp_before = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));
            assert_ne!(
                timestamp_before, new_time,
                "case {new_time}: timestamps must be different"
            );
            let expected_response =
                (new_time as i128).saturating_sub(timestamp_before as i128) as i64;

            let actual_response = evm
                .set_time(U64::from(new_time))
                .await
                .expect("failed setting timestamp");
            let timestamp_after = node
                .get_inner()
                .read()
                .map(|inner| inner.current_timestamp)
                .unwrap_or_else(|_| panic!("case {}: failed reading timestamp", new_time));

            assert_eq!(
                expected_response, actual_response,
                "case {new_time}: erroneous response"
            );
            assert_eq!(
                new_time, timestamp_after,
                "case {new_time}: timestamp was not set correctly",
            );
        }
    }
}