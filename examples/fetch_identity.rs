// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of substrate-subxt.
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
// along with substrate-subxt.  If not, see <http://www.gnu.org/licenses/>.

use codec::Encode;
pub use pallet_identity::Data;
use std::str::FromStr;
use substrate_subxt::{
    identity::{
        IdentityOfStoreExt,
        SuperOfStoreExt,
    },
    sp_runtime::AccountId32,
    ClientBuilder,
    DefaultNodeRuntime,
};

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let client = ClientBuilder::<DefaultNodeRuntime>::new()
        .set_url("wss://westend-rpc.polkadot.io")
        .build()
        .await?;
    let account_id =
        AccountId32::from_str("5DUGF2j9PsCdQ9okZfRoiCgbSeq3TUEAwuHvBQ3qjX4Nz4oR")?;
    if let Some(registration) = client.identity_of(account_id, None).await? {
        let display_name = String::from_utf8(registration.info.display.encode()).unwrap();
        println!(
            "{} -> display_name",
            display_name
        );
    };

    let account_id =
        AccountId32::from_str("5FbpwTP4usJ7dCFvtwzpew6NfXhtkvZH1jY4h6UfLztyD3Ng")?;
    if let None = client.identity_of(account_id.clone(), None).await? {
        if let Some((parent_account, data)) = client.super_of(account_id, None).await? {
            let sub_account_name = String::from_utf8(data.encode()).unwrap();
            println!("{} -> sub_account_name", sub_account_name);
            if let Some(registration) = client.identity_of(parent_account, None).await? {
                let parent_account_name = String::from_utf8(registration.info.display.encode()).unwrap();
                println!(
                    "{} -> parent_account_name",
                    parent_account_name
                );
            };
        }
    };

    Ok(())
}
