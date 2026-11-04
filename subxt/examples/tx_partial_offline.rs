#![allow(missing_docs)]
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::dev;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ### Downloading FULL metadata from a Substrate node
// subxt metadata --url ws://localhost:9944 -f bytes > artifacts/polkadot_metadata_v0.38.scale
// #[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata_v0.38.scale")]
// pub mod polkadot {}

// ### Downloading a SMALL metadata from a Substrate node with specific pallets
// subxt metadata --url ws://localhost:9944 --pallets System,Utility -f bytes > artifacts/polkadot_metadata_small_v0.38.scale
#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata_small_v0.38.scale")]
pub mod node_runtime {}

type Call = node_runtime::runtime_types::rococo_runtime::RuntimeCall;
type SystemCall = node_runtime::runtime_types::frame_system::pallet::Call;

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    // Spawned tasks require things held across await points to impl Send,
    // so we use one to demonstrate that this is possible with `PartialExtrinsic`
    tokio::spawn(signing_batch_example()).await??;
    Ok(())
}

async fn signing_batch_example() -> Result<(), BoxedError> {
    let api = OnlineClient::<PolkadotConfig>::new().await?;

    // Runtime metadata is identical to the metadata used during API generation.
    assert!(node_runtime::is_codegen_valid_for(&api.metadata()));

    // Create a batch call with a few remarks
    let mut calls_for_batch: Vec<Call> = vec![];
    for n in 1..2 {
        let call = Call::System(SystemCall::remark_with_event {
            remark: format!("Hello {n}").as_bytes().to_vec(),
        });
        calls_for_batch.push(call);
    }

    // Create a batch call TX payload
    let call = node_runtime::tx().utility().batch(calls_for_batch);

    // Create partial offline tx, ready to be signed.
    let partial_tx = api
        .tx()
        .create_partial_signed_offline(&call, Default::default())?;

    // Fetch the payload that needs to be signed:
    let signer_payload = partial_tx.signer_payload();

    // *****
    // NOTE: Get the signature from an external `Signer`..
    // For the purpose of this test lets just use a test account:
    let alice = dev::alice();
    let signature: subxt_signer::sr25519::Signature = alice.sign(&signer_payload).into();
    // *****

    // Sign the transaction.
    let tx = partial_tx
        .sign_with_address_and_signature(&alice.public_key().to_address(), &signature.into());

    // Dry run the transaction.
    let dry_res = tx.validate().await;
    println!("dry_res: {:?}", dry_res);

    // Submit it.
    tx.submit_and_watch()
        .await?
        .wait_for_finalized_success()
        .await?;

    Ok(())
}
