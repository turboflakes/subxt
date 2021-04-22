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

//! Implements support for the pallet_identity module.

use crate::frame::{
    balances::Balances,
    system::System,
};
use codec::{
    Decode,
    Encode,
};

use std::{
    fmt::Debug,
    marker::PhantomData,
};

pub use pallet_identity::{
    Data,
    Registration,
};

/// The subset of the `frame_identity::Trait` that a client must implement.
#[module]
pub trait Identity: System + Balances {}

/// Information that is pertinent to identify the entity behind an account
#[derive(Clone, Encode, Decode, Debug, Copy, PartialEq, Eq, Store)]
pub struct IdentityOfStore<T: Identity> {
    #[store(returns = Option<Registration<<T as Balances>::Balance>>)]
    ///  Account to retrieve identity from
    pub account: T::AccountId,
    /// Marker for the runtime
    pub _runtime: PhantomData<T>,
}

/// The super-identity of an alternative "sub" identity together with its name,
/// within that context. If the account is not some other account's sub-identity,
/// then just None.
#[derive(Clone, Encode, Decode, Debug, Copy, PartialEq, Eq, Store)]
pub struct SuperOfStore<T: Identity> {
    #[store(returns = Option<(T::AccountId, Data)>)]
    ///  Account to retrieve identity from
    pub account: T::AccountId,
    /// Marker for the runtime
    pub _runtime: PhantomData<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extrinsic::{
            PairSigner,
            Signer,
        },
        tests::{
            test_node_process,
            TestRuntime,
        },
        Error,
    };
    use assert_matches::assert_matches;

    use sp_keyring::AccountKeyring;

    #[async_std::test]
    async fn test_identity_of_account() -> Result<(), Error> {
        env_logger::try_init().ok();
        let alice = PairSigner::<TestRuntime, _>::new(AccountKeyring::Alice.pair());
        let test_node_proc = test_node_process().await;
        let client = test_node_proc.client();
        let store = IdentityOfStore {
            account: alice.account_id().clone(),
            _runtime: PhantomData,
        };
        let registration = client.fetch(&store, None).await?;
        assert_matches!(registration, None);
        Ok(())
    }

    #[async_std::test]
    async fn test_data_encode() -> Result<(), Error> {
        let encoded = Data::Raw(b"Hello".to_vec()).encode();
        assert_eq!(String::from_utf8(encoded).unwrap(), "\u{6}Hello");
        Ok(())
    }
}
