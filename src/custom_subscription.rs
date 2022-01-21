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

use jsonrpsee::core::{
    client::Subscription,
    DeserializeOwned,
};
use sp_core::{
    storage::{
        StorageChangeSet,
        StorageKey,
    },
    twox_128,
};
use sp_runtime::{traits::Header, Digest, DigestItem};
use sp_consensus_babe::{AuthorityIndex, digests::PreDigest};
use std::{collections::VecDeque};
use codec::Decode;

use crate::{
    error::Error,
    events::{
        EventsDecoder,
        RawEvent,
    },
    rpc::Rpc,
    Config,
    Event,
    Phase,
};

/// Event subscription simplifies filtering a storage change set stream for
/// events of interest.
pub struct CustomEventSubscription<'a, T: Config> {
    block_reader: BlockReader<'a, T>,
    block: Option<T::Hash>,
    extrinsic: Option<usize>,
    event: Option<(&'static str, &'static str)>,
    events: VecDeque<(T::BlockNumber, Option<AuthorityIndex>, RawEvent)>,
    finished: bool,
}

enum BlockReader<'a, T: Config> {
    Decoder {
        subscription: CustomEventStorageSubscription<T>,
        decoder: &'a EventsDecoder<T>,
    },
    /// Mock event listener for unit tests
    #[cfg(test)]
    Mock(Box<dyn Iterator<Item = (T::Hash, T::BlockNumber, Option<AuthorityIndex>, Result<Vec<(Phase, RawEvent)>, Error>)>>),
}

impl<'a, T: Config> BlockReader<'a, T> {
    async fn next(&mut self) -> Option<(T::Hash, T::BlockNumber, Option<AuthorityIndex>, Result<Vec<(Phase, RawEvent)>, Error>)> {
        match self {
            BlockReader::Decoder {
                subscription,
                decoder,
            } => {
                let (header, change_set) = subscription.next().await?;
                let events: Result<Vec<_>, _> = change_set
                    .changes
                    .into_iter()
                    .filter_map(|(_key, change)| {
                        Some(decoder.decode_events(&mut change?.0.as_slice()))
                    })
                    .collect();

                let flattened_events = events.map(|x| x.into_iter().flatten().collect());

                let authority = self.get_authority_index(header.clone());
                
                Some((change_set.block, *header.number(), authority, flattened_events))
            }
            #[cfg(test)]
            BlockReader::Mock(it) => it.next(),
        }
    }

    fn get_authority_index(&mut self, header: T::Header) -> Option<AuthorityIndex> {
        match header.digest() {
            Digest { logs } => {
                for digests in logs.iter() {
                    match digests {
                        DigestItem::PreRuntime(_, data) => {
                            if let Some(pre) = PreDigest::decode(&mut &data[..]).ok() {
                                return Some(pre.authority_index())
                            } else {
                                return None
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        None
    }
}

impl<'a, T: Config> CustomEventSubscription<'a, T> {
    /// Creates a new event subscription.
    pub fn new(
        subscription: CustomEventStorageSubscription<T>,
        decoder: &'a EventsDecoder<T>,
    ) -> Self {
        Self {
            block_reader: BlockReader::Decoder {
                subscription,
                decoder,
            },
            block: None,
            extrinsic: None,
            event: None,
            events: Default::default(),
            finished: false,
        }
    }

    /// Only returns events contained in the block with the given hash.
    pub fn filter_block(&mut self, block: T::Hash) {
        self.block = Some(block);
    }

    /// Only returns events from block emitted by extrinsic with index.
    pub fn filter_extrinsic(&mut self, block: T::Hash, ext_index: usize) {
        self.block = Some(block);
        self.extrinsic = Some(ext_index);
    }

    /// Filters events by type.
    pub fn filter_event<E: Event>(&mut self) {
        self.event = Some((E::PALLET, E::EVENT));
    }

    /// Gets a tuple with next event and respective block number and authority_index
    pub async fn next(&mut self) -> Option<Result<(T::BlockNumber, Option<AuthorityIndex>, RawEvent), Error>> {
        loop {
            if let Some((block_number, authority, raw_event)) = self.events.pop_front() {
                return Some(Ok((block_number, authority, raw_event)))
            }
            if self.finished {
                return None
            }
            // always return None if subscription has closed
            let (received_hash, block_number, authority, events) = self.block_reader.next().await?;
            if let Some(hash) = self.block.as_ref() {
                if &received_hash == hash {
                    self.finished = true;
                } else {
                    continue
                }
            }

            match events {
                Err(err) => return Some(Err(err)),
                Ok(raw_events) => {
                    for (phase, raw) in raw_events {
                        if let Some(ext_index) = self.extrinsic {
                            if !matches!(phase, Phase::ApplyExtrinsic(i) if i as usize == ext_index)
                            {
                                continue
                            }
                        }
                        if let Some((module, variant)) = self.event {
                            if raw.pallet != module || raw.variant != variant {
                                continue
                            }
                        }
                        self.events.push_back((block_number, authority, raw));
                    }
                }
            }
        }
    }
}

pub(crate) struct SystemEvents(StorageKey);

impl SystemEvents {
    pub(crate) fn new() -> Self {
        let mut storage_key = twox_128(b"System").to_vec();
        storage_key.extend(twox_128(b"Events").to_vec());
        log::debug!("Events storage key {:?}", hex::encode(&storage_key));
        Self(StorageKey(storage_key))
    }
}

impl From<SystemEvents> for StorageKey {
    fn from(key: SystemEvents) -> Self {
        key.0
    }
}

/// Event subscription to only fetch finalized storage changes.
pub struct CustomFinalizedEventStorageSubscription<T: Config> {
    rpc: Rpc<T>,
    subscription: Subscription<T::Header>,
    storage_changes: VecDeque<(T::Header, StorageChangeSet<T::Hash>)>,
    storage_key: StorageKey,
}

impl<T: Config> CustomFinalizedEventStorageSubscription<T> {
    /// Creates a new finalized event storage subscription.
    pub fn new(rpc: Rpc<T>, subscription: Subscription<T::Header>) -> Self {
        Self {
            rpc,
            subscription,
            storage_changes: Default::default(),
            storage_key: SystemEvents::new().into(),
        }
    }

    /// Gets the next change_set.
    pub async fn next(&mut self) -> Option<(T::Header, StorageChangeSet<T::Hash>)> {
        loop {
            if let Some((header, storage_change)) = self.storage_changes.pop_front() {
                return Some((header, storage_change))
            }
            let header: T::Header =
                read_subscription_response("HeaderSubscription", &mut self.subscription)
                    .await?;

            let storage_change_set: Vec<StorageChangeSet<T::Hash>> = self.rpc
                .query_storage_at(&[self.storage_key.clone()], Some(header.hash()))
                .await
                .ok()?;
            
            let mut tmp: Vec<(T::Header, StorageChangeSet<T::Hash>)> = Vec::new();

            for storage_change in storage_change_set {
                tmp.push((header.clone(), storage_change));
            }

            self.storage_changes.extend(tmp);
        }
    }
}

/// Wrapper over imported and finalized event subscriptions.
pub enum CustomEventStorageSubscription<T: Config> {
    // /// Events that are InBlock
    // Imported(Subscription<StorageChangeSet<T::Hash>>),
    /// Events that are Finalized
    Finalized(CustomFinalizedEventStorageSubscription<T>),
}

impl<T: Config> CustomEventStorageSubscription<T> {
    /// Gets the next change_set from the subscription.
    pub async fn next(&mut self) -> Option<(T::Header, StorageChangeSet<T::Hash>)> {
        match self {
            // Self::Imported(event_sub) => {
            //     read_subscription_response("StorageChangeSetSubscription", event_sub)
            //         .await
            // }
            Self::Finalized(event_sub) => event_sub.next().await,

        }
    }
}

async fn read_subscription_response<T>(
    sub_name: &str,
    sub: &mut Subscription<T>,
) -> Option<T>
where
    T: DeserializeOwned,
{
    match sub.next().await {
        Some(Ok(next)) => Some(next),
        Some(Err(e)) => {
            log::error!("Subscription {} failed: {:?} dropping", sub_name, e);
            None
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DefaultConfig;
    use sp_core::H256;

    fn named_event(event_name: &str) -> RawEvent {
        RawEvent {
            data: sp_core::Bytes::from(Vec::new()),
            pallet: event_name.to_string(),
            variant: event_name.to_string(),
            pallet_index: 0,
            variant_index: 0,
        }
    }

    #[async_std::test]
    /// test that filters work correctly, and are independent of each other
    async fn test_filters() {
        let mut events = vec![];
        // create all events
        for block_hash in [H256::from([0; 32]), H256::from([1; 32])] {
            for phase in [
                Phase::Initialization,
                Phase::ApplyExtrinsic(0),
                Phase::ApplyExtrinsic(1),
                Phase::Finalization,
            ] {
                for event in [named_event("a"), named_event("b")] {
                    events.push((block_hash, phase.clone(), event))
                }
            }
        }
        // set variant index so we can uniquely identify the event
        events.iter_mut().enumerate().for_each(|(idx, event)| {
            event.2.variant_index = idx as u8;
        });

        let half_len = events.len() / 2;

        for block_filter in [None, Some(H256::from([1; 32]))] {
            for extrinsic_filter in [None, Some(1)] {
                for event_filter in [None, Some(("b", "b"))] {
                    let mut subscription: CustomEventSubscription<DefaultConfig> =
                        CustomEventSubscription {
                            block_reader: BlockReader::Mock(Box::new(
                                vec![
                                    (
                                        events[0].0,
                                        1,
                                        Some(1),
                                        Ok(events
                                            .iter()
                                            .take(half_len)
                                            .map(|(_, phase, event)| {
                                                (phase.clone(), event.clone())
                                            })
                                            .collect()),
                                    ),
                                    (
                                        events[half_len].0,
                                        1,
                                        Some(1),
                                        Ok(events
                                            .iter()
                                            .skip(half_len)
                                            .map(|(_, phase, event)| {
                                                (phase.clone(), event.clone())
                                            })
                                            .collect()),
                                    ),
                                ]
                                .into_iter(),
                            )),
                            block: block_filter,
                            extrinsic: extrinsic_filter,
                            event: event_filter,
                            events: Default::default(),
                            finished: false,
                        };
                    let mut expected_events = events.clone();
                    if let Some(hash) = block_filter {
                        expected_events.retain(|(h, _, _)| h == &hash);
                    }
                    if let Some(idx) = extrinsic_filter {
                        expected_events.retain(|(_, phase, _)| matches!(phase, Phase::ApplyExtrinsic(i) if *i as usize == idx));
                    }
                    if let Some(name) = event_filter {
                        expected_events.retain(|(_, _, event)| event.pallet == name.0);
                    }

                    for expected_event in expected_events {
                        assert_eq!(
                            subscription.next().await.unwrap().unwrap().2,
                            expected_event.2
                        );
                    }
                    assert!(subscription.next().await.is_none());
                }
            }
        }
    }
}
