// subxt metadata --url wss://sys.turboflakes.io:443/statemine -f bytes > statemine_metadata.scale
// subxt codegen --file statemine_metadata.scale | rustfmt --edition=2018 --emit=stdout > statemine_metadata.rs

use std::{collections::BTreeMap, convert::TryFrom, time::Instant};
use subxt::{
    utils::{AccountId32, H256},
    OnlineClient, PolkadotConfig,
};

#[subxt::subxt(runtime_metadata_path = "../artifacts/statemine_metadata.scale")]
pub mod statemine {}

use statemine::runtime_types::{
    bounded_collections::bounded_vec::BoundedVec, sp_consensus_slots::Slot,
};

//
// First NewDesiredCandidates Event happened at block 1570428
//
// https://statemine.subscan.io/event?address=&module=collatorselection&event=newdesiredcandidates&startDate=&endDate=&startBlock=&endBlock=&timeType=date&version=9420
// block_number: 1570428
// block_hash: 0x64af750320fbd3b6e6dde6a3d2c131f1eb14b3d9faf9059f7b54f4266f150342
//
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let api =
        OnlineClient::<PolkadotConfig>::from_url("wss://sys.turboflakes.io:443/statemine").await?;

    let mut collator_stats = BTreeMap::new();

    let start_from_block_number = 1570428_u32;
    let latest_block = api.blocks().at_latest().await?;
    let latest_block_number = latest_block.number();

    println!("Start counting authored blocks per collator from #{start_from_block_number} to #{latest_block_number}\n");

    let mut block_number_optional: Option<u32> = Some(start_from_block_number.into());
    while let Some(block_number) = block_number_optional {
        if latest_block_number == block_number {
            block_number_optional = None;
        } else {
            if let Some(block_number_hash) = api.rpc().block_hash(Some(block_number.into())).await?
            {
                let author_index = find_author_index(api.clone(), block_number_hash).await?;

                // identify collator from authority index
                let collators_query = statemine::storage().session().validators();
                if let Some(collators) = api
                    .storage()
                    .at(block_number_hash)
                    .fetch(&collators_query)
                    .await?
                {
                    let i = usize::try_from(author_index)?;
                    if let Some(collator) = collators.get(i) {
                        // check if collator is invulnerable
                        let invulnerables_query =
                            statemine::storage().collator_selection().invulnerables();
                        let is_invulnerable = if let Some(BoundedVec(invulnerables)) = api
                            .storage()
                            .at(block_number_hash)
                            .fetch(&invulnerables_query)
                            .await?
                        {
                            invulnerables.contains(&collator)
                        } else {
                            false
                        };

                        collator_stats
                            .entry(collator_str(collator, is_invulnerable))
                            .and_modify(|x| *x += 1)
                            .or_insert(1);
                    }
                }
            }

            if block_number % 10000 == 0 {
                println!("Going at block_number: {block_number}");
            }

            block_number_optional = Some(block_number + 1);
        }
    }

    // sort and print collators by blocks authored
    let mut v = Vec::from_iter(collator_stats.clone());
    v.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
    for c in v.iter() {
        println!("{}: {}", c.0, c.1);
    }
    let total_blocks: u32 = v.iter().map(|(_, a)| a).sum();
    println!(
        "\nFinish processing {total_blocks} blocks in {:?}",
        start.elapsed()
    );

    Ok(())
}

async fn find_author_index(
    api: OnlineClient<PolkadotConfig>,
    block_hash: H256,
) -> Result<u32, Box<dyn std::error::Error>> {
    let authorities_query = statemine::storage().aura().authorities();
    if let Some(BoundedVec(authorities)) = api
        .storage()
        .at(block_hash)
        .fetch(&authorities_query)
        .await?
    {
        let current_slot_query = statemine::storage().aura().current_slot();
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
