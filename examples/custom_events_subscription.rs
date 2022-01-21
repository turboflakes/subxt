// Copyright 2019-2022 Parity Technologies (UK) Ltd.
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
    CustomEventSubscription
};
use codec::Decode;

#[subxt::subxt(
    runtime_metadata_path = "examples/polkadot_metadata.scale",
    generated_type_derives = "Clone, Debug"
)]
pub mod polkadot {}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let api = ClientBuilder::new()
        .set_url("wss://rpc.polkadot.io:443")
        .build()
        .await?
        .to_runtime_api::<polkadot::RuntimeApi<DefaultConfig, DefaultExtra<DefaultConfig>>>();

    let active_validators = api.storage().session().validators(None).await?;

    let client = api.client.clone();
    let sub = client.rpc().subscribe_finalized_events_with_block().await?;
    let decoder = client.events_decoder();
    let mut sub = CustomEventSubscription::<DefaultConfig>::new(sub, &decoder);
    sub.filter_event::<polkadot::system::events::ExtrinsicSuccess>();
    while let Some(result) = sub.next().await {
        if let Ok((block_number, authority, raw_event)) = result {

            if let Some(authority_index) = authority {
                let i: usize = authority_index.try_into().unwrap();
                if let Some(account_id) = active_validators.get(i) {
                    println!("block: {} author: {}", block_number, account_id);
                    match polkadot::system::events::ExtrinsicSuccess::decode(&mut &raw_event.data[..]) {
                        Ok(event) => {
                            println!("Successfully decoded event {:?}", event);
                        }
                        _ => ()
                    };
                }
            }
        }
    }

    Ok(())
}
