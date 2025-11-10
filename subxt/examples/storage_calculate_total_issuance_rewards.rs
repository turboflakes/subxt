#![allow(missing_docs)]

use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use subxt::backend::{legacy::LegacyRpcMethods, rpc::RpcClient};
use subxt::utils::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};
use tokio::time::Instant;

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/asset_hub_polkadot_metadata.scale")]
pub mod runtime {}

use crate::runtime::runtime_types::bounded_collections::bounded_btree_map::BoundedBTreeMap;
use crate::runtime::runtime_types::sp_arithmetic::per_things::Perbill;
use crate::runtime::staking::storage::types::payee::Payee;

const OUTPUT_FILENAME: &str = "total_issuance_rewards.csv";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // Create a new API client, configured to talk to Polkadot nodes.
    // let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io/asset-hub-polkadot").await?;

    // First, create a raw RPC client:
    let rpc_client = RpcClient::from_url("wss://rpc.ibp.network/asset-hub-polkadot").await?;

    // Use this to construct our RPC methods:
    let rpc = LegacyRpcMethods::<PolkadotConfig>::new(rpc_client.clone().into());

    // We can use the same client to drive our full Subxt interface too:
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client.clone()).await?;

    let mut missing_rewards: BTreeMap<AccountId32, u128> = BTreeMap::new();

    // Start at block 10265699 where the first EraPaid event was emitted
    let start_block_number = 10265699_u32;

    // End block could be the next block after the one containing the EraPaid event where runtime 2000002 is already enacted
    let end_block_number = 10333743_u32;

    // The old and new total issuance were collected from
    // https://github.com/polkadot-fellows/runtimes/pull/998/files#diff-02eb31199deb234b1df06a7173bf2f4694dbf9e34139e20063d89a5efd86246aR305
    let old_fixed_total_issuance: u128 = 5_216_342_402_773_185_773;
    let new_fixed_total_issuance: u128 = 15_011_657_390_566_252_333;

    println!(
        "Iterating over {} blocks",
        end_block_number - start_block_number
    );
    let mut latest_block_number_processed: Option<u32> = Some(start_block_number);
    while let Some(block_number) = latest_block_number_processed {
        if block_number == end_block_number {
            latest_block_number_processed = None;
        } else {
            if let Some(block_hash) = rpc.chain_get_block_hash(Some(block_number.into())).await? {
                let events = api.events().at(block_hash).await?;

                // Calculate self-stake missing rewards only at the end of the era
                if let Some(era_paid_event) =
                    events.find_first::<runtime::staking::events::EraPaid>()?
                {
                    // Get era total points
                    let query = runtime::storage()
                        .staking()
                        .eras_reward_points(era_paid_event.era_index);
                    let result = api.storage().at(block_hash).fetch(&query).await?;

                    if let Some(era_reward_points) = result {
                        let BoundedBTreeMap(rewarded_validators) = era_reward_points.individual;
                        for (stash, points) in rewarded_validators {
                            // Fetch validator commission rate
                            let query = runtime::storage().staking().validators(stash.clone());
                            let result = api.storage().at(block_hash).fetch(&query).await?;

                            if let Some(validator_prefs) = result {
                                let Perbill(commission) = validator_prefs.commission;

                                // Fetch self-stake included in rewards calculation
                                let query = runtime::storage()
                                    .staking()
                                    .eras_stakers_overview(era_paid_event.era_index, stash.clone());
                                let result = api.storage().at(block_hash).fetch(&query).await?;
                                if let Some(eras_stakers_overview) = result {
                                    // Calculate total amount of rewards for the validator based on the points earned on the era
                                    let total_reward = (era_paid_event.validator_payout
                                        * points as u128)
                                        / era_reward_points.total as u128;
                                    let validator_commission_reward =
                                        (total_reward * commission as u128) / 1_000_000_000_u128;
                                    let total_nominators_reward =
                                        total_reward - validator_commission_reward;

                                    let validator_own_reward = (eras_stakers_overview.own
                                        * total_nominators_reward)
                                        / eras_stakers_overview.total;

                                    let validator_reward_received =
                                        validator_own_reward + validator_commission_reward;

                                    // Apply the new fixed_total_issuance rate on top of the validator reward received to calculate the expected reward
                                    let validator_reward_expected = (new_fixed_total_issuance
                                        * validator_reward_received)
                                        / old_fixed_total_issuance;

                                    let vmr = missing_rewards.entry(stash.clone()).or_default();
                                    *vmr += validator_reward_expected - validator_reward_received;

                                    // To calculate each nominator missed reward we iterate over eras_stakers_paged
                                    let query =
                                        runtime::storage().staking().eras_stakers_paged_iter2(
                                            era_paid_event.era_index,
                                            stash.clone(),
                                        );
                                    let mut iter = api.storage().at(block_hash).iter(query).await?;

                                    while let Some(Ok(data)) = iter.next().await {
                                        let exposure = data.value.0;
                                        for individual_exposure in exposure.others {
                                            let nominator_reward_received = (individual_exposure
                                                .value
                                                * total_nominators_reward)
                                                / eras_stakers_overview.total;
                                            let nominator_reward_expected =
                                                (new_fixed_total_issuance
                                                    * nominator_reward_received)
                                                    / old_fixed_total_issuance;
                                            let nmr = missing_rewards
                                                .entry(individual_exposure.who.clone())
                                                .or_default();
                                            *nmr += nominator_reward_expected
                                                - nominator_reward_received;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Print progress every 10%
            let v = ((end_block_number - block_number) as f64
                / (end_block_number - start_block_number) as f64)
                * 100_f64;
            if v % 10_f64 == 0_f64 {
                println!(
                    "Processed {}%, {} blocks still to go.",
                    100_f64 - v,
                    end_block_number - block_number
                );
            }

            latest_block_number_processed = Some(block_number + 1);
        }
    }

    let file = File::create(OUTPUT_FILENAME)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "stash,reward,destination")?;

    // Fetch reward destination account, convert address formats to default Polkadot address and export data as csv
    let polkadot = Ss58AddressFormat::custom(0);
    for (stash, reward) in &missing_rewards {
        let stash_address = sp_core::crypto::AccountId32::new(stash.0);
        let query = runtime::storage().staking().payee(stash.clone());
        let result = api.storage().at_latest().await?.fetch(&query).await?;
        if let Some(payee) = result {
            match payee {
                Payee::Account(destination) => {
                    let destination_address = sp_core::crypto::AccountId32::new(destination.0);
                    writeln!(
                        writer,
                        "{},{},{}",
                        stash_address.to_ss58check_with_version(polkadot),
                        reward,
                        destination_address.to_ss58check_with_version(polkadot)
                    )?;
                }
                _ => {
                    writeln!(
                        writer,
                        "{},{},{}",
                        stash_address.to_ss58check_with_version(polkadot),
                        reward,
                        stash_address.to_ss58check_with_version(polkadot)
                    )?;
                }
            }
        }
    }
    writer.flush()?;

    println!(
        "Calculated total missed rewards: {} DOT for {} accounts ({:?})",
        (missing_rewards.values().sum::<u128>() as f64 / 10_000_000_000_f64),
        missing_rewards.len(),
        start.elapsed()
    );

    Ok(())
}
