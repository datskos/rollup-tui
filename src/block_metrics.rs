use crate::types::NetworkMetrics;
use alloy_rpc_types::Block;
use chrono::Utc;
use std::collections::{HashSet, VecDeque};

const WINDOW_SECONDS: u64 = 60;

#[derive(Default)]
pub struct BlockMetricsBuffer {
    network: String,
    buffer: VecDeque<BlockInfo>,
    seen: HashSet<u64>,
    total_txs: usize,
    total_gas: u64,
    total_data: u64,
}

impl BlockMetricsBuffer {
    pub fn new(network: String) -> Self {
        Self {
            network,
            ..Default::default()
        }
    }

    pub fn get_metrics(&mut self) -> NetworkMetrics {
        self.update();
        match (self.buffer.front(), self.buffer.back()) {
            (Some(first), Some(last)) if last.timestamp > first.timestamp => {
                let span = Utc::now().timestamp() as u64 - first.timestamp;
                //let span = last.timestamp - first.timestamp;
                NetworkMetrics {
                    network: self.network.clone(),
                    block: last.bn,
                    gps: self.total_gas as f64 / span as f64,
                    tps: self.total_txs as f64 / span as f64,
                    dps: self.total_data as f64 / span as f64,
                }
            }
            _ => NetworkMetrics {
                network: self.network.clone(),
                ..Default::default()
            },
        }
    }

    pub fn add_block(&mut self, block: &Block) {
        if let Some(block_info) = BlockInfo::try_from_block(block) {
            self.add_block_info(block_info);
        }
    }

    fn add_block_info(&mut self, block: BlockInfo) {
        if self.seen.contains(&block.bn) {
            return;
        }

        self.update();
        self.buffer.push_back(block.clone());
        self.total_txs += block.txs;
        self.total_gas += block.gas;
        if let Some(size) = block.size {
            self.total_data += size;
        }
        self.seen.insert(block.bn);
    }

    fn update(&mut self) {
        let current_time = Utc::now().timestamp() as u64;
        while let Some(front_block) = self.buffer.front() {
            if current_time - front_block.timestamp >= WINDOW_SECONDS {
                let block = self.buffer.pop_front().unwrap();
                self.total_txs -= block.txs;
                self.total_gas -= block.gas;
                if let Some(size) = block.size {
                    self.total_data -= size;
                }
                self.seen.remove(&block.bn);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct BlockInfo {
    bn: u64,
    gas: u64,
    size: Option<u64>,
    timestamp: u64,
    txs: usize,
}

impl BlockInfo {
    fn try_from_block(block: &Block) -> Option<Self> {
        match (block.header.number, block.header.gas_used) {
            (Some(bn), gas) if gas < u64::MAX as u128 => Some(Self {
                bn,
                gas: gas as u64,
                size: block.size.map(|s| s.as_limbs()[0]),
                timestamp: block.header.timestamp,
                txs: block.transactions.len(),
            }),
            _ => None,
        }
    }
}
