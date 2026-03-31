#![allow(missing_docs)]

use codec::Encode;
use sp_core::crypto::{Ss58AddressFormat, Ss58Codec};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use subxt::backend::{legacy::LegacyRpcMethods, rpc::RpcClient};
use subxt::utils::{AccountId32, MultiAddress};
use subxt::{OnlineClient, PolkadotConfig};
use tokio::time::Instant;

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(
    runtime_metadata_path = "../artifacts/asset_hub_polkadot_metadata.scale",
    derive_for_all_types = "Clone, PartialEq, codec::Encode, codec::Decode"
)]
pub mod runtime {}

use crate::runtime::runtime_types::bounded_collections::bounded_btree_map::BoundedBTreeMap;
use crate::runtime::runtime_types::sp_arithmetic::per_things::Perbill;
use crate::runtime::staking::storage::types::payee::Payee;
use crate::runtime::utility::calls::types::with_weight::Weight;

type Call = crate::runtime::runtime_types::asset_hub_polkadot_runtime::RuntimeCall;
type UtilityCall = crate::runtime::runtime_types::pallet_utility::pallet::Call;
type PreimageCall = crate::runtime::runtime_types::pallet_preimage::pallet::Call;

const OUTPUT_FILENAME: &str = "total_issuance_rewards.csv";
const CALLS_FILENAME: &str = "validated_batch_calls.csv";

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

    // End block is one block after the block where runtime 2000002 is enacted
    let end_block_number = 10344188_u32;

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

    // Fetch block weights
    let query = runtime::constants().system().block_weights();
    let block_weights = api.constants().at(&query)?;

    let max_extrinsic_weight = block_weights
        .per_class
        .normal
        .max_extrinsic
        .expect("Max extrinsic weights not found.");

    let file = File::create(CALLS_FILENAME)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "#,call_data")?;

    // Create a treasury.spend_call for each missing reward
    let mut calls_for_batch: Vec<Call> = build_calls_for_batch(&missing_rewards)?;

    // 1. Add maximum of spend_calls in a single call of type utility.batch_all
    // 2. Export the call_data to a csv file
    let mut iteration = Some(1);
    while let Some(x) = iteration {
        let pending_calls =
            validate_calls_for_batch(&api, &mut calls_for_batch, max_extrinsic_weight.clone())
                .await?;

        if calls_for_batch.len() > 0 {
            let preimage_call = Call::Preimage(PreimageCall::note_preimage {
                bytes: calls_for_batch.encode(),
            });

            let hex_call_data = to_hex(&preimage_call.encode());
            writeln!(writer, "{}:{}", x, hex_call_data)?;
        }

        if let Some(next_calls) = pending_calls {
            calls_for_batch = next_calls;
            iteration = Some(x + 1);
        } else {
            iteration = None;
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

pub fn to_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes.as_ref()))
}

fn build_calls_for_batch(
    data: &BTreeMap<AccountId32, u128>,
) -> Result<Vec<Call>, Box<dyn std::error::Error>> {
    type TreasuryCall = crate::runtime::runtime_types::pallet_treasury::pallet::Call;

    let mut calls_for_batch: Vec<Call> = vec![];

    for (account, reward) in data.into_iter() {
        let beneficiary: MultiAddress<AccountId32, ()> = MultiAddress::Id(account.clone());
        let call = Call::Treasury(TreasuryCall::spend_local {
            beneficiary,
            amount: *reward,
        });
        calls_for_batch.push(call);
    }
    Ok(calls_for_batch)
}

pub async fn validate_calls_for_batch(
    api: &OnlineClient<PolkadotConfig>,
    calls: &mut Vec<Call>,
    max_weight: Weight,
) -> Result<Option<Vec<Call>>, Box<dyn std::error::Error>> {
    let mut pending_calls = Vec::new();

    loop {
        let batch_call = Call::Utility(UtilityCall::batch_all {
            calls: calls.clone(),
        });

        let preimage_call = Call::Preimage(PreimageCall::note_preimage {
            bytes: batch_call.encode(),
        });

        match validate_call_via_tx_payment(&api, preimage_call.clone(), max_weight.clone()).await {
            Ok(_) => {
                if pending_calls.is_empty() {
                    println!("Preimage validated with {} calls successfully", calls.len());
                    return Ok(None);
                } else {
                    println!(
                        "Preimage validated with {} calls successfully. Pending calls: {}",
                        calls.len(),
                        pending_calls.len()
                    );
                    return Ok(Some(pending_calls));
                }
            }
            Err(err) => {
                let err_str = err.to_string();
                if err_str == "MaxWeightExceeded" {
                    println!("Preimage with {} calls got weight exceeded", calls.len());
                    // NOTE: If there's only one call left, we can't split it further.
                    // This should never happen, as a single payout should always be able to fit
                    // within the extrinsic weight limit.
                    if calls.len() == 1 {
                        return Err("MaxWeightExceededForOneExtrinsic".into());
                    }

                    // Remove half of the calls to speed up the process
                    let mut split_point = calls.len() / 2;
                    // Ensure split_point is even and try to fit one more if it's odd
                    if split_point % 2 != 0 {
                        split_point = if split_point > 1 { split_point + 1 } else { 1 };
                    }
                    let mut removed = calls.split_off(split_point);
                    pending_calls.append(&mut removed);
                } else {
                    return Err(err);
                }
            }
        }
    }
}

async fn validate_call_via_tx_payment(
    api: &OnlineClient<PolkadotConfig>,
    call: Call,
    max_weight: Weight,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime_api_call = runtime::apis()
        .transaction_payment_call_api()
        .query_call_info(call, 0);

    let result = api
        .runtime_api()
        .at_latest()
        .await?
        .call(runtime_api_call)
        .await?;

    println!("Call weight: {:?}, max_weight: {:?}", result, max_weight);

    if result.weight.ref_time > max_weight.ref_time
        || result.weight.proof_size > max_weight.proof_size
    {
        return Err("MaxWeightExceeded".into());
    }

    Ok(())
}
