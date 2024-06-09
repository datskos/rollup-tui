use crate::block_streamer::BlockStreamer;
use crate::networks::read_networks;
use crate::tui::tui;
use tokio::spawn;
use tokio::sync::mpsc::channel;

mod block_metrics;
mod block_streamer;
mod networks;
mod tui;
mod types;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let networks = read_networks("config/networks.json")?;
    let (tx, rx) = channel(8);
    for network in &networks {
        let mut streamer = BlockStreamer::new(network.clone(), tx.clone()).await?;
        spawn(async move {
            let _ = streamer.start().await;
        });
    }

    tui(networks, rx).await?;
    Ok(())
}
