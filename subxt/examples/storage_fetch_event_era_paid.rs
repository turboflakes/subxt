#![allow(missing_docs)]

use subxt::backend::{legacy::LegacyRpcMethods, rpc::RpcClient};
use subxt::{
    OnlineClient, PolkadotConfig,
    codec::{Decode, Encode},
};

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/asset_hub_kusama_metadata_small.scale")]
pub mod node_runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new API client, configured to talk to Polkadot nodes.
    // let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io/kusama").await?;

    // First, create a raw RPC client:
    let rpc_client = RpcClient::from_url("wss://rpc.turboflakes.io/kusama").await?;

    // Use this to construct our RPC methods:
    let rpc = LegacyRpcMethods::<PolkadotConfig>::new(rpc_client.clone().into());

    // We can use the same client to drive our full Subxt interface too:
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client.clone()).await?;

    // Clone the static metadata (v1003000) so it can be reassigned later if needed.
    let static_metadata = api.metadata().clone();

    let block_number = 11450717_u32;

    if let Some(block_hash) = rpc.chain_get_block_hash(Some(block_number.into())).await? {
        println!("block_hash: {:?}", block_hash);

        let events = api.events().at(block_hash).await?;

        if let Some(era_paid_event) =
            events.find_first::<node_runtime::session::events::EraPaid>()?
        {
            println!("era_paid_event: {:?}", era_paid_event);
            // get
        }
    }

    Ok(())
}
