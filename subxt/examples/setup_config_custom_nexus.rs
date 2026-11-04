#![allow(missing_docs)]
use codec::Encode;
use polkadot_sdk::sp_crypto_hashing;

use subxt::{
    OnlineClient,
    config::{
        Config, DefaultExtrinsicParams, DefaultExtrinsicParamsBuilder, Hasher, SubstrateConfig,
        substrate::SubstrateHeader,
    },
    utils::{H256, MultiAddress},
};

#[subxt::subxt(runtime_metadata_path = "../artifacts/polkadot_metadata_full.scale")]
pub mod runtime {}

/// A type that can hash values using the keccak_256 algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
pub struct Keccak256;

impl Hasher for Keccak256 {
    type Output = H256;
    fn hash(s: &[u8]) -> Self::Output {
        sp_crypto_hashing::keccak_256(s).into()
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum NexusConfig {}

impl Config for NexusConfig {
    type Hash = <SubstrateConfig as Config>::Hash;
    type AccountId = <SubstrateConfig as Config>::AccountId;
    type Address = MultiAddress<Self::AccountId, ()>;
    type Signature = <SubstrateConfig as Config>::Signature;
    type Hasher = Keccak256;
    type Header = SubstrateHeader<u32, Self::Hasher>;
    type ExtrinsicParams = NexusExtrinsicParams<Self>;
    type AssetId = u32;
}

/// A struct representing the signed extra and additional parameters required
/// to construct a transaction for a nexus node.
pub type NexusExtrinsicParams<T> = DefaultExtrinsicParams<T>;

/// A builder which leads to [`NexusExtrinsicParams`] being constructed.
pub type NexusExtrinsicParamsBuilder<T> = DefaultExtrinsicParamsBuilder<T>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<NexusConfig>::from_url("wss://nexus.ibp.network").await?;

    // Subscribe to all finalized blocks:
    let mut blocks_sub = api.blocks().subscribe_finalized().await?;

    // For each block, print details about the `TransferKeepAlive` transactions we are interested in.
    while let Some(block) = blocks_sub.next().await {
        let block = block?;
        let block_number = block.header().number;
        let block_hash = block.hash();
        println!("Block #{block_number}  ({:?}):", block_hash);

        let extrinsics = block.extrinsics().await?;
        println!("Extrinsics: {:?}", extrinsics.len());
    }

    Ok(())
}
