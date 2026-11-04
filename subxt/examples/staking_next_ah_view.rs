// staking_next_ah_view.rs
//
// Example to verify types, events and storage related to staking from RC
// based on https://hackmd.io/7PiBrGxxRG2ib-WRZYJZhQ
//
// Generate metadata:
// subxt codegen --url ws://127.0.0.1:9966 | rustfmt --edition=2018 --emit=stdout > artifacts/ah_next_metadata.rs
// subxt metadata --url ws://127.0.0.1:9966 -f bytes > artifacts/ah_next_metadata.scale
//
// Run:
// cargo run --example staking_next_ah_view
//
//
#![allow(missing_docs)]
use log::info;
use std::env;
use subxt::{OnlineClient, PolkadotConfig};

#[subxt::subxt(
    runtime_metadata_path = "../artifacts/ah_next_metadata.scale",
    derive_for_all_types = "PartialEq, Clone"
)]
pub mod ah_node {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "info");
    env_logger::try_init().unwrap_or_default();

    // Create a client to use:
    let api = OnlineClient::<PolkadotConfig>::from_url("ws://127.0.0.1:9966").await?;

    // Subscribe to all finalized blocks:
    let mut blocks_sub = api.blocks().subscribe_best().await?;
    // For each block, print a bunch of information about it:
    while let Some(block) = blocks_sub.next().await {
        let block = block?;

        let block_number = block.header().number;
        let block_hash = block.hash();

        info!("");
        info!("");
        info!("AH.block: {} hash: {:?}", block_number, block_hash);

        // Log each of the events in the block:
        let events = block.events().await?;
        for event in events.iter() {
            let event = event?;

            let pallet = event.pallet_name();
            let variant = event.variant_name();
            if pallet == "MultiBlock"
                || pallet == "MultiBlockVerifier"
                || pallet == "MultiBlockSigned"
                || pallet == "MultiBlockUnsigned"
                || pallet == "StakingNextRcClient"
                || (pallet == "Staking"
                    && (variant == "EraPaid"
                        || variant == "SessionRotated"
                        || variant == "PagedElectionProceeded"))
            {
                let field_values = event.field_values()?;

                info!("AH.event: {pallet}::{variant}: {field_values}");
            } else {
                let field_values = event.field_values()?;

                info!("AH.event__: {pallet}::{variant}: {field_values}");
            }
        }

        // Storage items to track:
        //
        // the current planned era
        let storage_query = ah_node::storage().staking().current_era();
        let current_era = api.storage().at(block_hash).fetch(&storage_query).await?;

        // the active era
        let storage_query = ah_node::storage().staking().active_era();
        let active_era = api.storage().at(block_hash).fetch(&storage_query).await?;

        // the starting index of the active era
        let storage_query = ah_node::storage()
            .staking()
            .eras_start_session_index(active_era.clone().unwrap().index);
        let eras_start_session_index = api.storage().at(block_hash).fetch(&storage_query).await?;

        // the basic state of the election provider
        let storage_query = ah_node::storage().multi_block().current_phase();
        let phase = api.storage().at(block_hash).fetch(&storage_query).await?;
        let storage_query = ah_node::storage().multi_block().round();
        let round = api.storage().at(block_hash).fetch(&storage_query).await?;

        let storage_query = ah_node::storage()
            .multi_block()
            .paged_voter_snapshot_hash_iter();
        let mut results = api.storage().at(block_hash).iter(storage_query).await?;
        while let Some(Ok(kv)) = results.next().await {
            // println!("Decoded key(s): {:?}", kv.keys);
            // println!("Key bytes: 0x{}", hex::encode(&kv.key_bytes));
            // println!("Value: {:?}", kv.value);
        }

        let storage_query = ah_node::storage()
            .multi_block_verifier()
            .queued_solution_score();
        let queued_score = api.storage().at(block_hash).fetch(&storage_query).await?;

        let signed_submissions = if round.is_none() {
            None
        } else {
            let storage_query = ah_node::storage()
                .multi_block_signed()
                .sorted_scores(round.clone().unwrap());
            api.storage().at(block_hash).fetch(&storage_query).await?
        };

        info!(
            "AH.staking: currentEra={:?}, activeEra={:?}, erasStartSessionIndex({:?})={:?}",
            current_era,
            active_era.clone(),
            active_era.unwrap().index,
            eras_start_session_index
        );
        info!(
            "AH.multiBlock: phase={:?}, round={:?}, snapshotRange=, queuedScore={:?}, signedSubmissions={:?}",
            phase, round, queued_score, signed_submissions
        );
    }

    Ok(())
}
