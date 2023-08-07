use subxt::{events::Events, OnlineClient, PolkadotConfig};
use std::time::Instant;

#[subxt::subxt(runtime_metadata_path = "../artifacts/westend_metadata.scale")]
pub mod polkadot {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new API client, configured to talk to Polkadot nodes.
    let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io:443/westend-subway").await?;

    println!("Test 1: Query all system accounts 3 times and measure the overall duration");
    for n in 1..4 {

        let start = Instant::now();
        // Build a storage query to iterate over account information.
        let storage_query = polkadot::storage().system().account_root();

        // Get back an iterator of results (here, we are fetching 10 items at
        // a time from the node, but we always iterate over one at a time).
        let mut results = api
            .storage()
            .at_latest()
            .await?
            .iter(storage_query, 10)
            .await?;

        
        let mut x = 0;
        while let Some(_) = results.next().await? {
            x += 1;
        }
        println!(
            "Run ({n}): {x} accounts loaded in {:?}",
            start.elapsed()
        );
    }

    println!("Test 2: Query 500 blocks and count overall events and measure the overall duration");
    for n in 1..11 {
        let start = Instant::now();
        let mut x = 0;
        for block_number in 16940000_u32..16940501_u32 {

            if let Some(block_hash) = api.rpc().block_hash(Some(block_number.into())).await? {
                let metadata = api.rpc().metadata_at_version(14).await?;
                let events = Events::new_from_client(metadata, block_hash, api.clone()).await?;
                x += events.iter().count();
            }

        }
        println!(
            "Run ({n}): {x} events loaded in {:?}",
            start.elapsed()
        );
    }

    Ok(())
}
