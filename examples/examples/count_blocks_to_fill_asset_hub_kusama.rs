// subxt metadata --url wss://sys.turboflakes.io:443/statemine -f bytes > asset_hub_kusama_metadata.scale
// subxt codegen --file asset_hub_kusama_metadata.scale | rustfmt --edition=2018 --emit=stdout > asset_hub_kusama_metadata.rs
// cargo run --example count_blocks_to_fill_asset_hub_kusama

use std::time::Instant;
use subxt::{
    utils::{AccountId32, H256},
    OnlineClient, PolkadotConfig,
};

#[subxt::subxt(runtime_metadata_path = "../artifacts/asset_hub_kusama_metadata.scale")]
pub mod node {}

use node::runtime_types::{bounded_collections::bounded_vec::BoundedVec, sp_consensus_slots::Slot};

//
// Lookup for NewDesiredCandidates Event and count blocks for each collator since the event was last seen
//
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let api =
        OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io/statemine").await?;

    let start_block_number = find_start_block(api.clone(), 4808220).await?;
    let _last_block_number = find_desired_candidates_filled(api.clone(), start_block_number).await?;
    
    println!(
        "\nFinish in {:?}",
        start.elapsed()
    );

    Ok(())
}

async fn find_start_block(
    api: OnlineClient<PolkadotConfig>,
    start_at_block_number: u32,
) -> Result<u32, Box<dyn std::error::Error>> {
    println!("Start looking for NewDesiredCandidates event..");
    let mut block_number = start_at_block_number;
    loop {
        if let Some(block_hash) = api.rpc().block_hash(Some(block_number.into())).await? {
            let events = api.events().at(block_hash).await?;
            if events
                .find_first::<node::collator_selection::events::NewDesiredCandidates>()?
                .is_some()
            {
                println!(
                    "NewDesiredCandidates event seen at block #{block_number} -> https://polkadot.js.org/apps/?rpc=wss://sys.turboflakes.io:443/statemint#/explorer/query/{block_number}",
                );
                break;
            } else {
                block_number += 1;
            }
        }
    }

    Ok(block_number)
}

async fn find_desired_candidates_filled(
    api: OnlineClient<PolkadotConfig>,
    start_at_block_number: u32,
) -> Result<u32, Box<dyn std::error::Error>> {
    println!("Check when maximum desired candidates match candidates submitted..");
    let mut block_number = start_at_block_number;
    loop {
        if let Some(block_hash) = api.rpc().block_hash(Some(block_number.into())).await? {
            let desired_candidates_addr = node::storage().collator_selection().desired_candidates();
            if let Some(dc) = api
                .storage()
                .at(block_hash)
                .fetch(&desired_candidates_addr)
                .await?
            {
                let candidates_addr = node::storage().collator_selection().candidates();
                if let Some(BoundedVec(candidates)) =
                    api.storage().at(block_hash).fetch(&candidates_addr).await?
                {
                    if candidates.len() as u32 == dc {
                        println!(
                            "Maximum of desired candidates {dc} reached at block #{block_number}, total blocks needed: {:}", block_number - start_at_block_number
                        );
                        break;
                    }
                }
            }
        }
        block_number += 1;
    }

    Ok(block_number)
}

async fn find_author_index(
    api: OnlineClient<PolkadotConfig>,
    block_hash: H256,
) -> Result<u32, Box<dyn std::error::Error>> {
    let authorities_query = node::storage().aura().authorities();
    if let Some(BoundedVec(authorities)) = api
        .storage()
        .at(block_hash)
        .fetch(&authorities_query)
        .await?
    {
        let current_slot_query = node::storage().aura().current_slot();
        if let Some(Slot(slot)) = api
            .storage()
            .at(block_hash)
            .fetch(&current_slot_query)
            .await?
        {
            let author_index = (slot % authorities.len() as u64) as u32;
            return Ok(author_index);
        }
    }

    Err(format!(
        "Author index not found for block_hash: {block_hash}"
    ))?
}

fn collator_str(acc: &AccountId32, is_invulnerable: bool) -> String {
    if is_invulnerable {
        format!("* {}", acc.to_string())
    } else {
        acc.to_string()
    }
}
