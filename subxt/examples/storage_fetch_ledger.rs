#![allow(missing_docs)]
use subxt::{OnlineClient, PolkadotConfig, utils::AccountId32};
use subxt_signer::sr25519::dev;

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(
    runtime_metadata_path = "../artifacts/latest/asset_hub_polkadot_metadata_small.scale"
)]
pub mod polkadot {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new API client, configured to talk to Polkadot nodes.
    let api =
        OnlineClient::<PolkadotConfig>::from_url("wss://asset-hub-polkadot.rpc.turboflakes.io")
            .await?;
    let account: AccountId32 = dev::alice().public_key().into();

    // Build a storage query to access account information.
    let storage_query = polkadot::storage().system().account();

    // Use that query to access a storage entry, fetch a result and decode the value.
    // The static address knows that fetching requires a tuple of one value, an
    // AccountId32.
    let client_at = api.storage().at_latest().await?;
    let account_info = client_at
        .entry(storage_query)?
        .fetch((account.clone(),))
        .await?
        .decode()?;

    // The static address that we got from the subxt macro knows the expected input
    // and return types, so it is decoded into a static type for us.
    println!("Alice: {account_info:?}");

    // Fetch staking ledger from an account
    let addr = polkadot::storage().staking().ledger();

    let api_at = api.storage().at_latest().await?;
    let result = api_at
        .entry(addr)?
        .fetch((account.clone(),))
        .await?
        .decode()?;

    println!("ledger: {result:?}");

    Ok(())
}
