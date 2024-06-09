#[derive(Clone, Debug, Default)]
pub struct NetworkMetrics {
    pub network: String,
    pub block: u64,
    pub gps: f64,
    pub tps: f64,
    pub dps: f64,
}

#[derive(Clone, Debug)]
pub struct Log {
    pub network: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum BlockMessage {
    UpdateNetwork(NetworkMetrics),
    #[allow(dead_code)] // TODO(george): send & display logs in UI
    Log(Log),
}
