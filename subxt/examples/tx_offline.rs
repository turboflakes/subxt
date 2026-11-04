//! Construct and submit a transaction.
use codec::Decode;
use subxt::tx::Payload;
use subxt::utils::to_hex;
use subxt::{Error, OnlineClient, PolkadotConfig};
use subxt::{
    OfflineClient,
    config::substrate::{SpecVersionForRange, SubstrateConfig},
};
use subxt_signer::sr25519::dev;

// Generate an interface that we can use from the node's metadata.
#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata.scale")]
mod polkadot {}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Load metadata from the local .scale file (the same one used for codegen above).
    let metadata = {
        let bytes = include_bytes!("../../artifacts/polkadot_metadata_small.scale");
        subxt::Metadata::decode(&mut &bytes[..])?
    };

    // Build an OfflineClient. It needs:
    //   1. The metadata (keyed by spec version — we pick an arbitrary one since we're
    //      not really doing block-historic lookups here).
    //   2. A mapping of block number → spec version (we cover the whole u64 range).
    let config = SubstrateConfig::builder()
        .set_metadata_for_spec_versions([(1, metadata.arc())])
        .set_spec_version_for_block_ranges([SpecVersionForRange {
            block_range: 0..u64::MAX,
            spec_version: 1,
            transaction_version: 1,
        }])
        .build();

    let offline_client = OfflineClient::new_with_config(config).at_block(0u64)?;

    // Build the call payload.
    let remark_call = polkadot::tx().system().remark("hello".into());

    // Encode just the call data: [pallet_index, call_index, ...SCALE-encoded args]
    let encoded: Vec<u8> = offline_client.tx().call_data(&remark_call)?;

    let hex_encoded = to_hex(&encoded);

    // Submit the transaction online.
    let api = OnlineClient::<PolkadotConfig>::new().await?;

    // Almost all actions are performed at an explicit block. Here we use
    // the current block at the time of running this.
    let at_block = api.at_current_block().await?;

    let signer = dev::alice();
    let tx = at_block
        .tx()
        .sign_and_submit_then_watch_default(&remark_call, &signer)
        .await?;

    // let signable = offline_client
    //     .tx()
    //     .create_signable_offline(&remark_call, Default::default())?;
    // let signer_payload = signable.signer_payload();

    println!("Encoded call data: {encoded:?} {hex_encoded}");
    Ok(())
}
