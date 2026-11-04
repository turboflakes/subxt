#![allow(missing_docs)]
use sp_core::Encode;
// use subxt::ext::parity_scale_codec::Encode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use subxt::dynamic::Value;
use subxt::tx::DynamicPayload;
use subxt::tx::Payload;
use subxt::tx::TxStatus;
use subxt::utils::AccountId32;
use subxt::utils::MultiAddress;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::SecretUri;
use subxt_signer::sr25519::Keypair;
use subxt_signer::sr25519::dev;
#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata_small.scale")]
pub mod polkadot {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let api = OnlineClient::<PolkadotConfig>::new().await?;
    // let api = OnlineClient::<PolkadotConfig>::from_url("wss://dev-relay.turboflakes.io").await?;
    let api =
        OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io:443/westend").await?;

    let proxied_account =
        AccountId32::from_str("5H9MMTLg18pLQh6b7JoDNDHTnPoQerpkEvwmvVPGX46fK5p4").unwrap();
    let proxy_signer: Keypair = get_keypair_from_seed_file()?;

    // let bob: AccountId32 = dev::bob().public_key().into();
    let transfer_tx = dynamic_transfer_payload(proxied_account.clone(), 10000);

    let chill_tx = dynamic_chill();
    // let metadata = api.metadata();

    // if let Some(enum_type) = metadata.types().resolve(103) {
    //     println!("Proxy type variants: {:?}", enum_type);
    // }

    // let proxy_data = transfer_tx.encode_call_data(&metadata)?;

    // Prepare some extrinsics. These are boxed so that they can live alongside each other.
    let txs = [
        // dynamic_remark(),
        // dynamic_remark_with_message("hello".to_string()),
        // balance_transfer(),
        // dynamic_transfer(10_000),
        // dynamic_proxy(bob.clone(), transfer_tx),
        dynamic_proxy(proxied_account.clone(), chill_tx), // remark(),
    ];

    for tx in txs {
        // let from = dev::alice();
        // api.tx()
        //     .sign_and_submit_then_watch_default(&tx, &from)
        //     .await?
        //     .wait_for_finalized_success()
        //     .await?;

        // println!("Submitted tx");
        let mut balance_transfer_progress = api
            .tx()
            .sign_and_submit_then_watch_default(&tx, &proxy_signer)
            .await?;

        while let Some(status) = balance_transfer_progress.next().await {
            match status? {
                // It's finalized in a block!
                TxStatus::InFinalizedBlock(in_block) => {
                    println!(
                        "Transaction {:?} is finalized in block {:?}",
                        in_block.extrinsic_hash(),
                        in_block.block_hash()
                    );

                    let _ = in_block.wait_for_success().await?;

                    let events = in_block.fetch_events().await?;
                    for event in events.iter() {
                        let event = event?;

                        let pallet = event.pallet_name();
                        let variant = event.variant_name();

                        if pallet == "Proxy" && variant == "ProxyExecuted" {
                            let field_values = event.field_values()?;

                            field_values
                                .values()
                                .for_each(|result_value| match &result_value {
                                    Value::<_> { value, context: _ } => {
                                        println!("Value: {}", value);
                                        match &value {
                                            subxt::ext::scale_value::ValueDef::Variant(variant) => {
                                                if variant.name == "Ok" {
                                                    println!("Proxy call succeeded!");
                                                } else if variant.name == "Err" {
                                                    println!(
                                                        "Proxy call failed: {:?}",
                                                        variant.values
                                                    );

                                                    let metadata = api.metadata();
                                                    println!("Metadata: {:?}", metadata);

                                                    // if let Some(enum_type) =
                                                    //     metadata.types().resolve(6)
                                                    // {
                                                    //     println!(
                                                    //         "Proxy type variants: {:?}",
                                                    //         enum_type
                                                    //     );
                                                    // }
                                                }
                                            }
                                            _ => println!("Unexpected result format"),
                                        }
                                    }
                                });
                        }
                    }
                }
                // Just log any other status we encounter:
                _ => {}
            }
        }
    }

    Ok(())
}

use subxt::ext::scale_value::serde::from_value;

// Define your custom type that matches the structure
#[derive(Debug, serde::Deserialize)]
struct DispatchErrorInfo {
    // Define fields that match the structure of the error
    module: Option<ModuleError>,
    // ... other fields
}

#[derive(Debug, serde::Deserialize)]
struct ModuleError {
    index: u8,
    error: [u8; 4],
    // ... other fields
}

// #[derive(Debug)]
// struct ProxyExecuted {
//     result: std::result::Result<(), DispatchError>,
// }

// // For the whole result
// #[derive(Debug, Serialize, Deserialize)]
// enum DispatchResult {
//     Ok(()),
//     Err(ModuleError),
// }

// // For error cases
// #[derive(Debug, Serialize, Deserialize)]
// enum DispatchError {
//     Module(ModuleError),
//     BadOrigin,
//     // Add other error variants as needed
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct ModuleError {
//     index: u8,
//     error: [u8; 4],
// }

// // Or for your specific Foo type
// #[derive(Debug, Serialize, Deserialize)]
// struct Foo {
//     // Define the fields that match the event data structure
//     // For example:
//     success: bool,
//     data: Vec<u8>,
// }

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct ErrT {
    module: String,
    call: String,
    value: String,
    doc: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Foo {
    Ok,
    Err(ErrT),
}

fn balance_transfer() -> Box<dyn subxt::tx::Payload> {
    let dest = dev::bob().public_key().into();
    Box::new(polkadot::tx().balances().transfer_allow_death(dest, 10_000))
}

fn remark() -> Box<dyn subxt::tx::Payload> {
    Box::new(polkadot::tx().system().remark(vec![1, 2, 3, 4, 5]))
}

pub fn dynamic_chill() -> DynamicPayload {
    use subxt::dynamic::{Value, tx};
    tx("Staking", "chill", vec![] as Vec<Value>)
}

fn dynamic_remark() -> Box<dyn subxt::tx::Payload> {
    use subxt::dynamic::{Value, tx};
    let tx_payload = tx("System", "remark", vec![Value::from_bytes("Hello")]);

    Box::new(tx_payload)
}

fn dynamic_remark_with_message(message: String) -> Box<dyn subxt::tx::Payload> {
    use subxt::dynamic::{Value, tx};

    let tx_payload = tx("System", "remark", vec![Value::from_bytes(message)]);

    Box::new(tx_payload)
}

fn dynamic_transfer(value: u128) -> Box<dyn subxt::tx::Payload> {
    use subxt::dynamic::{Value, tx};
    let bob: AccountId32 = dev::bob().public_key().into();
    let bob_bytes: &[u8] = bob.as_ref();

    let tx_payload = tx(
        "Balances",
        "transfer_keep_alive",
        vec![
            (
                "dest",
                Value::unnamed_variant("Id", vec![Value::from_bytes(bob_bytes)]),
            ),
            ("value", Value::u128(value)),
        ],
    );

    Box::new(tx_payload)
}

fn dynamic_transfer_payload(destination: AccountId32, value: u128) -> DynamicPayload {
    use subxt::dynamic::{Value, tx};
    // let bob: AccountId32 = dev::bob().public_key().into();
    let bob_bytes: &[u8] = destination.as_ref();

    tx(
        "Balances",
        "transfer_keep_alive",
        vec![
            (
                "dest",
                Value::unnamed_variant("Id", vec![Value::from_bytes(bob_bytes)]),
            ),
            ("value", Value::u128(value)),
        ],
    )
}

fn dynamic_proxy(
    proxied_account: AccountId32,
    call: DynamicPayload,
) -> Box<dyn subxt::tx::Payload> {
    use subxt::dynamic::{Value, tx};
    let proxied_account_bytes: &[u8] = proxied_account.as_ref();

    let tx_payload = tx(
        "Proxy",
        "proxy",
        vec![
            (
                "real",
                Value::unnamed_variant("Id", vec![Value::from_bytes(proxied_account_bytes)]),
            ),
            (
                "force_proxy_type",
                Value::unnamed_variant("Some", vec![Value::unnamed_variant("Any", vec![])]),
            ),
            ("call", call.into_value()),
            // (
            //     "call",
            //     Value::unnamed_variant(
            //         "RuntimeCall",
            //         vec![Value::unnamed_variant(
            //             "Balances",
            //             vec![Value::from_bytes(call)],
            //         )],
            //     ),
            // ),
        ],
    );

    Box::new(tx_payload)
}

/// Helper function to generate a keypair from the content of the seed file
pub fn get_keypair_from_seed_file() -> Result<Keypair, Box<dyn std::error::Error>> {
    // load data from seed file
    let data = std::fs::read_to_string(
        "/Users/paulo/Development/github.com/turboflakes/subxt/.private.seed",
    )?;

    // clear control characters from data
    let re = Regex::new(r"[\x00-\x1F]").unwrap();
    let data = re.replace_all(&data.trim(), "");

    // parse data into a secret
    let uri = SecretUri::from_str(&data)?;
    let keypair = Keypair::from_uri(&uri)?;
    Ok(keypair)
}

// let metadata = api.metadata();
// if let Some(enum_type) = metadata.types().resolve(237) {
//     info!("Proxy type variants: {:?}", enum_type);
// }
