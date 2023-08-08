use std::time::Instant;
use subxt::{events::Events, OnlineClient, PolkadotConfig};

#[subxt::subxt(runtime_metadata_path = "../artifacts/westend_metadata.scale")]
pub mod polkadot {}

async fn test_1(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url(url).await?;
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
        println!("Run ({n}): {x} accounts loaded in {:?}", start.elapsed());
    }

    Ok(())
}

async fn test_2(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url(url).await?;
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
        println!("Run ({n}): {x} events loaded in {:?}", start.elapsed());
    }

    Ok(())
}

async fn test_3(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url(url).await?;
    println!("Test 3: Query 1 block 10000 times and measure the overall duration");
    for n in 1..4 {
        let start = Instant::now();
        let mut x = 0;
        for block_number in 16940000_u32..16940001_u32 {
            for z in 1..10001 {
                if let Some(block_hash) = api.rpc().block_hash(Some(block_number.into())).await? {
                    let _block = api.rpc().block(Some(block_hash)).await?;
                    x += 1;
                }
            }
        }
        println!("Run ({n}): {x} blocks loaded in {:?}", start.elapsed());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "wss://rpc.turboflakes.io:443/westend-subway";

    let res = tokio::try_join!(
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url),
        test_3(url)
    );
    match res {
        Ok(_) => {
            // do something with the values
        }
        Err(err) => {
            println!("processing failed; error = {}", err);
        }
    }
    println!("Test finished!");
    Ok(())
}
