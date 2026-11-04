#![allow(missing_docs)]
use futures::StreamExt;
use std::{fs, str::FromStr};
use subxt::{
    PolkadotConfig, backend::unstable::UnstableBackend, client::OnlineClient,
    lightclient::LightClient, tx::TxStatus,
};
use subxt_signer::{SecretUri, sr25519::Keypair};
use tracing;

const ROCOCO_SPEC: &str = include_str!("../../artifacts/demo_chain_specs/rococo.json");

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/rococo_metadata_small.scale")]
pub mod polkadot {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer_keypair: Keypair = get_keypair_from_seed_file()?;

    // Create a new API client, configured to talk to Polkadot nodes.
    // let api = OnlineClient::<PolkadotConfig>::from_url("wss://rococo-rpc.polkadot.io:443").await?;

    // Initiate light client (smoldot)
    let (_, rpc) = LightClient::relay_chain(ROCOCO_SPEC)?;

    // NOTE: The latest RPC specs are implemented via UnstableBackend in Subxt which is the preferred way to connect to smoldot v0.18
    let (unstable_backend, mut driver) = UnstableBackend::builder().build(rpc);

    // Unstable backend needs manually driving at the moment see here:
    // https://github.com/paritytech/subxt/issues/1453#issuecomment-2011922808
    tokio::spawn(async move {
        while let Some(val) = driver.next().await {
            if let Err(e) = val {
                // Something went wrong driving unstable backend.
                println!("error driving unstable backend: {:?}", e);
                break;
            }
        }
    });

    // Create client from unstable backend (ie using new RPCs).
    let api = OnlineClient::<PolkadotConfig>::from_backend(unstable_backend.into())
        .await
        .expect("expect valid RPC connection");

    let remark_tx = polkadot::tx().system().remark_with_event(b"test".to_vec());

    let mut tx_progress = api
        .tx()
        .sign_and_submit_then_watch_default(&remark_tx, &signer_keypair)
        .await?;

    while let Some(status) = tx_progress.next().await {
        match status? {
            // It's finalized in a block!
            TxStatus::InFinalizedBlock(in_block) => {
                println!(
                    "Transaction {:?} is finalized in block {:?}",
                    in_block.extrinsic_hash(),
                    in_block.block_hash()
                );

                // grab the events and fail if no ExtrinsicSuccess event seen:
                let events = in_block.wait_for_success().await?;
                // We can look for events (this uses the static interface; we can also iterate
                // over them and dynamically decode them):
                let remark_event = events.find_first::<polkadot::system::events::Remarked>()?;

                if let Some(event) = remark_event {
                    println!("Remark success: {event:?}");
                } else {
                    println!("Failed to find Remarked Event");
                }
            }
            // Just log any other status we encounter:
            other => {
                println!("Status: {other:?}");
            }
        }
    }
    Ok(())
}

/// Helper function to generate a keypair from the content of the seed file
pub fn get_keypair_from_seed_file() -> Result<Keypair, Box<dyn std::error::Error>> {
    let data = fs::read_to_string("./artifacts/.private.seed")?;
    let uri = SecretUri::from_str(&data)?;
    Ok(Keypair::from_uri(&uri)?)
}
