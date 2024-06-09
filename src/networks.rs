use serde_derive::Deserialize;
use std::fs::File;
use std::io::BufReader;

#[derive(Clone, Debug, Deserialize)]
pub struct Network {
    pub name: String,
    pub label: String,
    pub http: String,
}

pub fn read_networks(file_path: &str) -> eyre::Result<Vec<Network>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let networks: Vec<Network> = serde_json::from_reader(reader)?;
    Ok(networks)
}
