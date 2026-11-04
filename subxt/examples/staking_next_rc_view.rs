// staking_next_rc_view.rs
//
// Example to verify types, events and storage related to staking from RC
// based on https://hackmd.io/7PiBrGxxRG2ib-WRZYJZhQ
//
// Generate metadata:
// subxt codegen --url ws://127.0.0.1:9955 | rustfmt --edition=2018 --emit=stdout > artifacts/rc_metadata.rs
// subxt metadata --url ws://127.0.0.1:9955 -f bytes > artifacts/rc_metadata.scale
//
// Run:
// cargo run --example staking_next_rc_view
//
//
#![allow(missing_docs)]
use log::info;
use std::env;
use subxt::backend::legacy::rpc_methods::StorageKey;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_core::utils::AccountId32;

// subxt codegen --url ws://127.0.0.1:9955 | rustfmt --edition=2018 --emit=stdout > artifacts/rc_metadata.rs
// subxt metadata --url ws://127.0.0.1:9955 -f bytes > artifacts/rc_metadata.scale
#[subxt::subxt(runtime_metadata_path = "../artifacts/rc_metadata.scale")]
pub mod rc_node {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "info");
    env_logger::try_init().unwrap_or_default();

    // Create a client to use:
    let api = OnlineClient::<PolkadotConfig>::from_url("ws://127.0.0.1:9955").await?;
    // Subscribe to all finalized blocks:
    let mut blocks_sub = api.blocks().subscribe_best().await?;
    // For each block, print a bunch of information about it:
    while let Some(block) = blocks_sub.next().await {
        let block = block?;

        let block_number = block.header().number;
        let block_hash = block.hash();

        info!("");
        info!("");
        info!("RC.block: {} hash: {:?}", block_number, block_hash);
        // Log each of the events in the block:
        let events = block.events().await?;
        for event in events.iter() {
            let event = event?;

            let pallet = event.pallet_name();
            if pallet == "StakingNextAhClient" || pallet == "Session" || pallet == "Historical" {
                let variant = event.variant_name();
                let field_values = event.field_values()?;

                info!("RC.event: {pallet}::{variant}: {field_values}");
            }
            // if let Ok(ev) = event.as_root_event::<rc_node::Event>() {
            //     // println!("RC::{ev:?}");
            // }
        }

        // println!("RC::Block #{block_number}:");
        // println!("  Hash: {block_hash}");

        // Storage items to track:
        //
        // current session index
        let storage_query = rc_node::storage().session().current_index();
        let current_index = api.storage().at(block_hash).fetch(&storage_query).await?;

        // whether the session pallet has a queued validator set within it
        let storage_query = rc_node::storage().session().queued_changed();
        let queued_changed = api.storage().at(block_hash).fetch(&storage_query).await?;

        // the range of historical session data that we have in the RC.
        let storage_query = rc_node::storage().historical().stored_range();
        let stored_range = api.storage().at(block_hash).fetch(&storage_query).await?;

        // whether there is a validator set queued in ah-client. for this we need to display only the id and the length of the set.
        let storage_query = rc_node::storage().staking_next_ah_client().validator_set();
        let has_queued_in_client = api.storage().at(block_hash).fetch(&storage_query).await?;

        // whether we have already passed a new validator set to session, and therefore in the next session rotation we want to pass this id to AH.
        let storage_query = rc_node::storage()
            .staking_next_ah_client()
            .next_session_changes_validators();
        let has_next_active_id = api.storage().at(block_hash).fetch(&storage_query).await?;

        // whether the AhClient pallet is blocked or not, useful for migration signal from the fellowship.
        let storage_query = rc_node::storage().staking_next_ah_client().is_blocked();
        let is_blocked = api.storage().at(block_hash).fetch(&storage_query).await?;

        info!(
            "RC.session: index={:?}, hasQueuedInSession={:?}, historicalRange={:?}",
            current_index, queued_changed, stored_range
        );
        if has_queued_in_client.is_some() {
            let has_queued_in_client = has_queued_in_client.unwrap();
            info!(
                "RC.stakingNextAhClient: hasQueuedInClient={:?}, length={:?}, hasNextActiveId={:?}, isBlocked={:?}",
                has_queued_in_client.0,
                has_queued_in_client.1.len(),
                has_next_active_id,
                is_blocked
            );
        } else {
            info!(
                "RC.stakingNextAhClient: hasQueuedInClient={:?}, hasNextActiveId={:?}, isBlocked={:?}",
                has_queued_in_client, has_next_active_id, is_blocked
            );
        }

        // // Look for events
        // let events = block.events().await?;
        // let ev =
        //     events.find_first::<rc_node::staking_next_ah_client::events::ValidatorSetReceived>()?;
        // if let Some(ev) = ev {
        //     println!("  - ValidatorSetReceived: value: {:?}", ev);
        // }

        // let ev = events
        //     .find_first::<rc_node::staking_next_ah_client::events::CouldNotMergeAndDropped>()?;
        // if let Some(ev) = ev {
        //     println!("  - CouldNotMergeAndDropped: value: {:?}", ev);
        // }

        // let ev = events
        //     .find_first::<rc_node::staking_next_ah_client::events::SetTooSmallAndDropped>()?;
        // if let Some(ev) = ev {
        //     println!("  - SetTooSmallAndDropped: value: {:?}", ev);
        // }
        //
        //
        //
        let validator_points_addr = rc_node::storage()
            .staking_next_ah_client()
            .validator_points_iter();
        let mut iter = api
            .storage()
            .at(block_hash)
            .iter(validator_points_addr)
            .await?;
        while let Some(Ok(storage_resp)) = iter.next().await {
            let stash = get_account_id_from_storage_key(storage_resp.clone().key_bytes);
            info!("storage_resp {} {:?}", stash, storage_resp.value);
        }
    }

    Ok(())
}

pub fn get_account_id_from_storage_key(key: StorageKey) -> AccountId32 {
    let s = &key[key.len() - 32..];
    let v: [u8; 32] = s.try_into().expect("slice with incorrect length");
    v.into()
}
