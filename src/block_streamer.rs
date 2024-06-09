use crate::block_metrics::BlockMetricsBuffer;
use crate::networks::Network;
use crate::types::BlockMessage;
use alloy_provider::{Provider, ProviderBuilder, ReqwestProvider};
use alloy_rpc_types::{Block, BlockTransactionsKind};
use futures::future::join_all;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::time;

const POLL_INTERVAL: Duration = Duration::from_millis(750);

pub struct BlockStreamer {
    provider: ReqwestProvider,
    metrics: BlockMetricsBuffer,
    tx: Sender<BlockMessage>,
}

/// BlockStreamer polls blocks from a given network and manages a windowed buffer of
/// block data to calculate average tx/s, gas/s, data/s.
///
/// Sends metrics to the provided channel
impl BlockStreamer {
    pub async fn new(network: Network, tx: Sender<BlockMessage>) -> eyre::Result<Self> {
        let rpc_url = network.http.parse()?;
        let provider = ProviderBuilder::new().on_http(rpc_url);
        let metrics = BlockMetricsBuffer::new(network.label.clone());
        Ok(Self { provider, tx, metrics })
    }

    pub async fn start(&mut self) -> eyre::Result<()> {
        let mut last_block = self.get_next_batch(None).await; // bootstrap
        loop {
            last_block = self.get_next_batch(last_block).await;
            let latest = self.metrics.get_metrics();
            self.tx.send(BlockMessage::UpdateNetwork(latest)).await?;
            time::sleep(POLL_INTERVAL).await;
        }
    }

    async fn get_next_batch(&mut self, previous_block: Option<u64>) -> Option<u64> {
        let latest_block_number = match self.provider.get_block_number().await {
            Ok(block_number) => block_number,
            Err(_) => return previous_block,
        };

        let previous_block = previous_block.unwrap_or_default().max(latest_block_number - 10);
        let fetch_futures = (previous_block + 1..=latest_block_number)
            .map(|bn| {
                let provider = self.provider.clone();
                async move { provider.get_block(bn.into(), BlockTransactionsKind::Hashes).await }
            })
            .collect::<Vec<_>>();

        let mut blocks: Vec<Block> =
            join_all(fetch_futures).await.into_iter().filter_map(Result::ok).flatten().collect();

        blocks.sort_by_key(|block| block.header.number);
        for block in blocks {
            self.metrics.add_block(&block);
        }

        Some(latest_block_number)
    }
}
