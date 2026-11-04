#![allow(missing_docs)]

use sp_core::Decode;
use subxt::backend::{legacy::LegacyRpcMethods, rpc::RpcClient};
use subxt::{
    // codec::{Decode, Encode},
    Metadata,
    OnlineClient,
    PolkadotConfig,
};

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/paseo_metadata_small.scale")]
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

    let block_number = 8595508_u32;

    if let Some(block_hash) = rpc.chain_get_block_hash(Some(block_number.into())).await? {
        println!("block_hash: {:?}", block_hash);

        println!("runtime_version: {:?}", api.runtime_version());

        // Assign _block_metadata_ to the api to be able to iterate events without the
        // `Error: Codec(Error { cause: None, desc: "Not enough data to fill buffer" })` being raised
        // for this particular block 24783412:

        let block_hash_metadata = rpc
            .chain_get_block_hash(Some((block_number).into()))
            .await?;
        let block_metadata = rpc.state_get_metadata(block_hash_metadata).await?;
        let metadata_bytes = block_metadata.into_raw();
        let metadata = Metadata::decode(&mut &metadata_bytes[..])?;
        api.set_metadata(metadata);

        let events = api.events().at(block_hash).await?;

        if let Some(new_session_event) =
            events.find_first::<node_runtime::session::events::NewSession>()?
        {
            println!("new_session_event: {:?}", new_session_event);
        }

        // // Reassign _static_metadata_ to the api so we can get on_chain_votes properly decoded without `Error: Metadata(IncompatibleCodegen)`
        // api.set_metadata(static_metadata);

        // // Fetch on chain votes
        // let on_chain_votes_addr = node_runtime::storage().para_inherent().on_chain_votes();
        // if let Some(backing_votes) = api
        //     .storage()
        //     .at(block_hash)
        //     .fetch(&on_chain_votes_addr)
        //     .await?
        // {
        //     println!("backing_votes: {:?}", backing_votes);
        // }
    }

    Ok(())
}
