#![allow(missing_docs)]
use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Instant;
use subxt::backend::legacy::LegacyRpcMethods;
use subxt::backend::rpc::RpcClient;
use subxt::config::substrate::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};

#[subxt::subxt(
    runtime_metadata_path = "../artifacts/polkadot_metadata.scale",
    derive_for_all_types = "PartialEq, Clone"
)]
pub mod node_runtime {}

use crate::node_runtime::runtime_types::bounded_collections::bounded_vec::BoundedVec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // Create a new API client, configured to talk to Polkadot nodes.
    // let api = OnlineClient::<PolkadotConfig>::from_insecure_url("ws://10.0.0.21:9951").await?;
    let rpc_client = RpcClient::from_insecure_url("ws://10.0.0.21:9951").await?;
    let rpc = LegacyRpcMethods::<PolkadotConfig>::new(rpc_client.clone());
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client.clone()).await?;

    // // Build a storage query to iterate over account information.
    // let storage_query = polkadot::storage().staking().nominators_iter();

    // // Get back an iterator of results (here, we are fetching 10 items at
    // // a time from the node, but we always iterate over one at a time).
    // let mut results = api.storage().at_latest().await?.iter(storage_query).await?;

    // while let Some(Ok(kv)) = results.next().await {
    //     println!("Keys decoded: {:?}", kv.keys);
    //     println!("Key: 0x{}", hex::encode(&kv.key_bytes));
    //     println!("Value: {:?}", kv.value);
    // }

    // BTreeMap<AccountId32, Vec<(AccountId32, u128, u32)>> = validator_stash : [(nominator_stash, nominator_total_stake, number_of_nominations)]
    let mut nominators_map: BTreeMap<AccountId32, Vec<(AccountId32, u128, u128)>> = BTreeMap::new();

    let block_number = 23299710_u32;
    if let Some(block_hash) = rpc.chain_get_block_hash(Some(block_number.into())).await? {
        let mut counter = 0;
        let storage_addr = node_runtime::storage().staking().nominators_iter();
        // let mut iter = api.storage().at_latest().await?.iter(storage_addr).await?;
        let mut iter = api.storage().at(block_hash).iter(storage_addr).await?;
        while let Some(Ok(storage_resp)) = iter.next().await {
            let nominator_stash = get_account_id_from_storage_key(storage_resp.key_bytes);
            let bonded_addr = node_runtime::storage()
                .staking()
                .bonded(&nominator_stash.clone());
            if let Some(controller) = api.storage().at_latest().await?.fetch(&bonded_addr).await? {
                let ledger_addr = node_runtime::storage().staking().ledger(&controller);
                let nominator_stake = if let Some(ledger) =
                    api.storage().at_latest().await?.fetch(&ledger_addr).await?
                {
                    ledger.total
                } else {
                    0
                };

                let BoundedVec(targets) = storage_resp.value.targets.clone();
                for target in targets.iter() {
                    let n = nominators_map.entry(target.clone()).or_insert(vec![]);
                    n.push((
                        nominator_stash.clone(),
                        nominator_stake,
                        targets.len().try_into().unwrap(),
                    ));
                }
            }
            counter += 1;
        }

        println!(
            "Total Nominators {} collected ({:?})",
            counter,
            start.elapsed()
        );

        let v = AccountId32::from_str("12gPFmRqnsDhc9C5DuXyXBFA23io5fSGtKTSAimQtAWgueD2").unwrap();

        if let Some(nominators) = nominators_map.get(&v) {
            let mut sorted = nominators.clone();
            sorted.sort_by(|(_, a, _), (_, b, _)| b.cmp(a));
            let mut counter = 0;
            println!("Total Nominators {} for {}", sorted.len(), v.to_string());
            for n in sorted {
                if counter == 50 {
                    break;
                }
                println!("{}, {}, {}", n.0.to_string(), n.1, n.2);
                counter += 1;
            }
        }
    }

    Ok(())
}

/// Storage key.
pub type StorageKey = Vec<u8>;

pub fn get_account_id_from_storage_key(key: StorageKey) -> AccountId32 {
    let s = &key[key.len() - 32..];
    let v: [u8; 32] = s.try_into().expect("slice with incorrect length");
    v.into()
}
