#![allow(missing_docs)]
use subxt::backend::{
    legacy::LegacyRpcMethods,
    rpc::RpcClient,
    rpc::reconnecting_rpc_client::{ExponentialBackoff, RpcClient as ReconnectingRpcClient},
};
use subxt::{OnlineClient, PolkadotConfig};

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/kusama_metadata_new.scale")]
pub mod node_runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new API client, configured to talk to Polkadot nodes.
    // let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.turboflakes.io/kusama").await?;

    // First, create a raw RPC client:
    let rpc_client = RpcClient::from_url("wss://rpc.turboflakes.io/kusama").await?;

    // let rpc_client = ReconnectingRpcClient::builder()
    //     .retry_policy(
    //         ExponentialBackoff::from_millis(100)
    //             .max_delay(time::Duration::from_secs(10))
    //             .take(10),
    //     )
    //     .build("wss://rpc.turboflakes.io/kusama".to_string())
    //     .await?;

    // Use this to construct our RPC methods:
    let rpc = LegacyRpcMethods::<PolkadotConfig>::new(rpc_client.clone().into());

    // We can use the same client to drive our full Subxt interface too:
    let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client.clone()).await?;

    // if let Some(backing_votes) = api
    //     .storage()
    //     .at_latest()
    //     .await?
    //     .fetch(&on_chain_votes_addr)
    //     .await?
    // {
    //     println!("backing_votes: {:?}", backing_votes);
    // }

    let block_number = 24783412_u32;
    // let block_number = 24786389_u32;

    // let block_hash_metadata = rpc
    //     .chain_get_block_hash(Some((block_number).into()))
    //     .await?;

    // let metadata = rpc.state_get_metadata(block_hash_metadata).await?;
    // println!("metadata_legacy: {:?}", metadata);

    // // assign metadata to the api
    // api.set_metadata(metadata);

    if let Some(block_hash) = rpc.chain_get_block_hash(Some(block_number.into())).await? {
        println!("block_hash: {:?}", block_hash);

        let events = api.events().at(block_hash).await?;

        if let Some(new_session_event) =
            events.find_first::<node_runtime::session::events::NewSession>()?
        {
            println!("new_session_event: {:?}", new_session_event);
        }

        // Fetch on chain votes
        let on_chain_votes_addr = node_runtime::storage().para_inherent().on_chain_votes();
        if let Some(backing_votes) = api
            .storage()
            .at(block_hash)
            .fetch(&on_chain_votes_addr)
            .await?
        {
            println!("backing_votes: {:?}", backing_votes);
        }
    }

    Ok(())
}
