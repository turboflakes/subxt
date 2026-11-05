//! This relies on an unstable smoldot JSON-RPC method, so it needs the `unstable-light-client`
//! feature. Run it twice and compare how long it takes to get the first finalized block:
//!
//! ```text
//! cargo run --example light_client_database --features "light-client unstable-light-client jsonrpsee"
//! ```
use futures::StreamExt;
use std::{path::Path, time::Instant};
use subxt::{
    OnlineClient, PolkadotConfig,
    lightclient::{ChainConfig, LightClient},
};

const POLKADOT_SPEC: &str = include_str!("../../artifacts/demo_chain_specs/polkadot.json");
const DATABASE_PATH: &str = "polkadot_light_client.db";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The lightclient logs are informative (look for the warp sync messages):
    tracing_subscriber::fmt::init();

    // Load the database saved by a previous run. If there isn't one (or it is invalid or
    // outdated) the light client simply does a normal sync.
    let database = if Path::new(DATABASE_PATH).exists() {
        println!("Loading light client database from {DATABASE_PATH}");
        std::fs::read_to_string(DATABASE_PATH)?
    } else {
        println!("No saved database found; syncing from the chain spec checkpoint");
        String::new()
    };

    let config = ChainConfig::chain_spec(POLKADOT_SPEC).set_database_content(database);
    let (_lightclient, polkadot_rpc) = LightClient::relay_chain(config)?;

    let started = Instant::now();
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(polkadot_rpc.clone()).await?;

    // Follow a few finalized blocks. The time to the first one reflects how long syncing took.
    let mut blocks = api.stream_blocks().await?.take(3);
    while let Some(block) = blocks.next().await {
        let block = block?;
        println!(
            "[{:.1?}] Polkadot height={:?} hash={:?}",
            started.elapsed(),
            block.number(),
            block.hash()
        );
    }

    // Export the database and save it for the next run.
    let database = polkadot_rpc.database_content(None).await?;
    std::fs::write(DATABASE_PATH, &database)?;
    println!(
        "Saved light client database ({} bytes) to {DATABASE_PATH}",
        database.len()
    );

    Ok(())
}
