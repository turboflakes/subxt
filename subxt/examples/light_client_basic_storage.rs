#![allow(missing_docs)]
use futures::StreamExt;
use std::collections::BTreeMap;
use std::time::Instant;
use subxt::{
    PolkadotConfig, backend::unstable::UnstableBackend, client::OnlineClient,
    lightclient::LightClient,
};

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata.scale")]
pub mod polkadot {}

use polkadot::{
    child_bounties::events::Claimed,
    runtime_types::{
        bounded_collections::bounded_vec::BoundedVec, pallet_child_bounties::ChildBountyStatus,
    },
    system::events::ExtrinsicFailed,
    system::events::ExtrinsicSuccess,
    utility::events::BatchCompleted,
    utility::events::BatchCompletedWithErrors,
};

const POLKADOT_SPEC: &str = include_str!("../../artifacts/demo_chain_specs/polkadot.json");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // The lightclient logs are informative:
    tracing_subscriber::fmt::init();

    let (_, polkadot_rpc) = LightClient::relay_chain(POLKADOT_SPEC)?;

    let (unstable_backend, mut driver) = UnstableBackend::builder().build(polkadot_rpc);

    tokio::spawn(async move {
        while let Some(val) = driver.next().await {
            if let Err(e) = val {
                // Something went wrong driving unstable backend.
                eprintln!("Error driving unstable backend: {e}");
                break;
            }
        }
    });

    // Create Subxt clients from this unstable backend (ie using new RPCs).
    let api = OnlineClient::<PolkadotConfig>::from_backend(unstable_backend.into()).await?;

    // let address = polkadot::storage().system().block_hash(22451810);
    // let address = polkadot::storage().system().parent_hash();

    // // if let Some(block_hash) = api.storage().at_latest().await?.fetch(&address).await?
    // // {

    // Subscribe to all finalized blocks:
    let mut blocks_sub = api.blocks().subscribe_finalized().await?;

    // For each block, print a bunch of information about it:
    // while let Some(block) = blocks_sub.next().await {
    //     let block = block?;
    //     let block_hash = block.hash();

    let mut data = BTreeMap::new();

    let address = polkadot::storage().child_bounties().child_bounties_iter();
    let mut iter = api.storage().at_latest().await?.iter(address).await?;
    while let Some(Ok(storage)) = iter.next().await {
        let id = get_child_bounty_id_from_storage_key(storage.key_bytes);
        // println!("__cb {:?}", id);

        // let a = polkadot::storage()
        //             .child_bounties()
        //             .child_bounty_descriptions(id.clone());

        // if let Some(BoundedVec(data)) = api.storage().at(block_hash.clone()).fetch(&a).await?
        // {
        //     // println!("__description {:?}", str(data));
        // };

        // println!("__description {} {}", id, description);

        data.entry(id).and_modify(|n| *n += 1).or_insert(1);
        // assert_eq!(data.len() as u32, data.iter().map(|(_, v)| v).sum::<u32>());
    }

    println!("__data {:?}", data);
    println!("__elapsed {:?}", start.elapsed());

    let address = polkadot::storage()
        .child_bounties()
        .child_bounty_descriptions_iter();
    let mut iter = api.storage().at_latest().await?.iter(address).await?;
    while let Some(Ok(storage)) = iter.next().await {
        let id = get_child_bounty_id_from_storage_key(storage.key_bytes);
        // println!("__cb {:?}", id);

        // let a = polkadot::storage()
        //             .child_bounties()
        //             .child_bounty_descriptions(id.clone());

        // if let Some(BoundedVec(data)) = api.storage().at(block_hash.clone()).fetch(&a).await?
        // {
        //     // println!("__description {:?}", str(data));
        // };

        // println!("__description {} {}", id, description);

        data.entry(id).and_modify(|n| *n += 1).or_insert(1);
        // assert_eq!(data.len() as u32, data.iter().map(|(_, v)| v).sum::<u32>());
    }

    println!("__data {:?}", data);
    println!("__elapsed {:?}", start.elapsed());
    //     break;
    // }
    println!("__elapsed {:?}", start.elapsed());

    // let mut data = BTreeMap::new();
    // let address = polkadot::storage().child_bounties().child_bounties_iter();
    // let mut iter = api.storage().at_latest().await?.iter(address).await?;
    // while let Some(Ok(storage)) = iter.next().await {
    //     let id = get_child_bounty_id_from_storage_key(storage.key_bytes);
    //     // println!("__cb {:?}", id);

    //     let a = polkadot::storage()
    //                 .child_bounties()
    //                 .child_bounty_descriptions(id.clone());

    //     if let Some(BoundedVec(data)) = api.storage().at_latest().await?.fetch(&a).await?
    //     {
    //         // println!("__description {:?}", str(data));
    //     };

    //     // println!("__description {} {}", id, description);

    //     data.entry(id).and_modify(|n| *n += 1).or_insert(1);
    //     assert_eq!(data.len() as u32, data.iter().map(|(_, v)| v).sum::<u32>());
    // }

    // println!("__data {:?}", data);
    // println!("__elapsed {:?}", start.elapsed());

    // let address = polkadot::storage().system().block_hash(22451810);
    // let address = polkadot::storage().system().parent_hash();

    // if let Some(block_hash) = api.storage().at_latest().await?.fetch(&address).await?
    // {

    //     let mut data = BTreeMap::new();
    //     let address = polkadot::storage().child_bounties().child_bounties_iter();
    //     let mut iter = api.storage().at(block_hash.clone()).iter(address).await?;
    //     while let Some(Ok(storage)) = iter.next().await {
    //         let id = get_child_bounty_id_from_storage_key(storage.key_bytes);
    //         // println!("__cb {:?}", id);

    //         let a = polkadot::storage()
    //                     .child_bounties()
    //                     .child_bounty_descriptions(id.clone());

    //         if let Some(BoundedVec(data)) = api.storage().at(block_hash.clone()).fetch(&a).await?
    //         {
    //             // println!("__description {:?}", str(data));
    //         };

    //         // println!("__description {} {}", id, description);

    //         data.entry(id).and_modify(|n| *n += 1).or_insert(1);
    //         assert_eq!(data.len() as u32, data.iter().map(|(_, v)| v).sum::<u32>());
    //     }

    //     println!("__data {:?}", data);
    //     println!("__elapsed {:?}", start.elapsed());

    // }

    Ok(())
}

pub fn get_child_bounty_id_from_storage_key(key: Vec<u8>) -> u32 {
    let s = &key[key.len() - 4..];
    let v: [u8; 4] = s.try_into().expect("slice with incorrect length");
    u32::from_le_bytes(v)
}

pub fn str(bytes: Vec<u8>) -> String {
    format!("{}", String::from_utf8(bytes).expect("Data not utf-8"))
}
