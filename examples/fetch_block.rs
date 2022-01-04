// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of subxt.
//
// subxt is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// subxt is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with subxt.  If not, see <http://www.gnu.org/licenses/>.

use subxt::{
    ClientBuilder,
    DefaultConfig,
    DefaultExtra,
    EventSubscription,
};

#[subxt::subxt(runtime_metadata_path = "examples/kusama_metadata.scale")]
pub mod kusama {}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let api = ClientBuilder::new()
        .set_url("wss://kusama-rpc.polkadot.io:443")
        .build()
        .await?
        .to_runtime_api::<kusama::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>>();

    // Looking for NewSession event at block 10819134
    let block_number = 10819134;

    let block_hash = api
        .client
        .rpc()
        .block_hash(Some(block_number.into()))
        .await?;

    // Get header
    let header = api
        .client
        .rpc()
        .header(block_hash)
        .await?;

    println!("Block header hash {:?}", header.unwrap().hash());

    if let Some(hash) = block_hash {
        println!("Block hash for block number {}: {}", block_number, hash);
    } else {
        println!("Block number {} not found.", block_number);
    }

    let sub = api.client.rpc().subscribe_finalized_events().await?;
    let decoder = api.client.events_decoder();
    let mut sub = EventSubscription::<DefaultConfig>::new(sub, &decoder);
    // sub.filter_event::<kusama::session::events::NewSession>();
    while let Some(result) = sub.next().await {
        if let Ok(raw) = result {
            println!("raw {:?}", raw);
            // match kusama::session::events::NewSession::decode(&mut &raw.data[..]) {
            //     Ok(event) => {
            //         info!("Successfully decoded event {:?}", event);
            //     }
            //     Err(e) => return Err(SkipperError::CodecError(e)),
            // }
        }
    }

    Ok(())
}
