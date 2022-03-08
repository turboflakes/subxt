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

//! Subscribing to events.

use crate::{
    error::BasicError,
    Client,
    Config,
};
use codec::Decode;
use derivative::Derivative;
use futures::{
    future::Either,
    stream::{
        self,
        BoxStream,
    },
    Future,
    FutureExt,
    Stream,
    StreamExt,
};
use jsonrpsee::core::client::Subscription;
use num_traits::One;
use sp_runtime::traits::Header;
use std::{
    marker::Unpin,
    task::Poll,
};

pub use super::{
    at,
    EventDetails,
    EventFilter,
    Events,
    EventsDecodingError,
    FilterEvents,
    RawEventDetails,
};

/// Subscribe to events from blocks.
///
/// **Note:** these blocks haven't necessarily been finalised yet; prefer
/// [`Events::subscribe_finalized()`] if that is important.
///
/// **Note:** This function is hidden from the documentation
/// and is exposed only to be called via the codegen. Thus, prefer to use
/// `api.events().subscribe()` over calling this directly.
#[doc(hidden)]
pub async fn subscribe<'a, T: Config, Evs: Decode + 'static>(
    client: &'a Client<T>,
) -> Result<EventSubscription<'a, EventSub<T::Header>, T, Evs>, BasicError> {
    let block_subscription = client.rpc().subscribe_blocks().await?;
    Ok(EventSubscription::new(client, block_subscription))
}

/// Subscribe to events from finalized blocks.
///
/// **Note:** This function is hidden from the documentation
/// and is exposed only to be called via the codegen. Thus, prefer to use
/// `api.events().subscribe_finalized()` over calling this directly.
#[doc(hidden)]
pub async fn subscribe_finalized<'a, T: Config, Evs: Decode + 'static>(
    client: &'a Client<T>,
) -> Result<EventSubscription<'a, FinalizedEventSub<'a, T::Header>, T, Evs>, BasicError> {
    // fetch the last finalised block details immediately, so that we'll get
    // events from this block onwards.
    let last_finalized_block_hash = client.rpc().finalized_head().await?;

    let mut last_finalized_block_number = client
        .rpc()
        .header(Some(last_finalized_block_hash))
        .await?
        .map(|h| *h.number());

    // The complexity here is because the finalized block subscription may skip over
    // some blocks, so here we attempt to ensure that we pay attention to every block
    // that was skipped over as well as the latest block we were told about.
    let block_subscription =
        client
            .rpc()
            .subscribe_finalized_blocks()
            .await?
            .flat_map(move |s| {
                // Get the header, or return a stream containing just the error. Our EventSubscription
                // stream will return `None` as soon as it hits an error like this.
                let header = match s {
                    Ok(header) => header,
                    Err(e) => return Either::Left(stream::once(async { Err(e.into()) })),
                };

                // This is the block number that we have returned details for already.
                let start_block_number =
                    last_finalized_block_number.unwrap_or(*header.number());
                // This latest finalized block number that we want to fetch all details up to.
                let end_block_number = *header.number();
                // On the next iteration, we'll start from this end block.
                last_finalized_block_number = Some(end_block_number);

                // Iterate over all of the previous blocks we need headers for, ignoring the current block
                // (which we already have the header info for):
                let prev_block_numbers = std::iter::from_fn({
                    let mut curr_block_number = start_block_number;
                    move || {
                        if curr_block_number == end_block_number {
                            None
                        } else {
                            curr_block_number = curr_block_number + One::one();
                            Some(curr_block_number)
                        }
                    }
                });

                // Produce a stream of all of the previous headers that finalization skipped over.
                let old_headers = stream::iter(prev_block_numbers)
                    .then(move |n| {
                        async move {
                            let hash = client.rpc().block_hash_internal(n).await?;
                            let header = client.rpc().header(hash).await?;
                            Ok::<_, BasicError>(header)
                        }
                    })
                    .filter_map(|h| async { h.transpose() });

                // Return a combination of any previous headers plus the new header.
                Either::Right(old_headers.chain(stream::once(async { Ok(header) })))
            });

    Ok(EventSubscription::new(client, Box::pin(block_subscription)))
}

/// A `jsonrpsee` Subscription. This forms a part of the `EventSubscription` type handed back
/// in codegen from `subscribe_finalized`, and is exposed to be used in codegen.
#[doc(hidden)]
pub type FinalizedEventSub<'a, Header> = BoxStream<'a, Result<Header, BasicError>>;

/// A `jsonrpsee` Subscription. This forms a part of the `EventSubscription` type handed back
/// in codegen from `subscribe`, and is exposed to be used in codegen.
#[doc(hidden)]
pub type EventSub<Item> = Subscription<Item>;

/// A subscription to events that implements [`Stream`], and returns [`Events`] objects for each block.
#[derive(Derivative)]
#[derivative(Debug(bound = "Sub: std::fmt::Debug"))]
pub struct EventSubscription<'a, Sub, T: Config, Evs: 'static> {
    finished: bool,
    client: &'a Client<T>,
    block_header_subscription: Sub,
    #[derivative(Debug = "ignore")]
    at: Option<
        std::pin::Pin<
            Box<dyn Future<Output = Result<Events<'a, T, Evs>, BasicError>> + Send + 'a>,
        >,
    >,
    _event_type: std::marker::PhantomData<Evs>,
}

impl<'a, Sub, T: Config, Evs: Decode, E: Into<BasicError>>
    EventSubscription<'a, Sub, T, Evs>
where
    Sub: Stream<Item = Result<T::Header, E>> + Unpin + 'a,
{
    fn new(client: &'a Client<T>, block_header_subscription: Sub) -> Self {
        EventSubscription {
            finished: false,
            client,
            block_header_subscription,
            at: None,
            _event_type: std::marker::PhantomData,
        }
    }

    /// Return only specific events matching the tuple of 1 or more event
    /// types that has been provided as the `Filter` type parameter.
    pub fn filter_events<Filter: EventFilter>(self) -> FilterEvents<'a, Self, T, Filter> {
        FilterEvents::new(self)
    }
}

impl<'a, T: Config, Sub: Unpin, Evs: Decode> Unpin
    for EventSubscription<'a, Sub, T, Evs>
{
}

// We want `EventSubscription` to implement Stream. The below implementation is the rather verbose
// way to roughly implement the following function:
//
// ```
// fn subscribe_events<T: Config, Evs: Decode>(client: &'_ Client<T>, block_sub: Subscription<T::Header>) -> impl Stream<Item=Result<Events<'_, T, Evs>, BasicError>> + '_ {
//     use futures::StreamExt;
//     block_sub.then(move |block_header_res| async move {
//         use sp_runtime::traits::Header;
//         let block_header = block_header_res?;
//         let block_hash = block_header.hash();
//         at(client, block_hash).await
//     })
// }
// ```
//
// The advantage of this manual implementation is that we have a named type that we (and others)
// can derive things on, store away, alias etc.
impl<'a, Sub, T, Evs, E> Stream for EventSubscription<'a, Sub, T, Evs>
where
    T: Config,
    Evs: Decode,
    Sub: Stream<Item = Result<T::Header, E>> + Unpin + 'a,
    E: Into<BasicError>,
{
    type Item = Result<Events<'a, T, Evs>, BasicError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // We are finished; return None.
        if self.finished {
            return Poll::Ready(None)
        }

        // If there isn't an `at` function yet that's busy resolving a block hash into
        // some event details, then poll the block header subscription to get one.
        if self.at.is_none() {
            match futures::ready!(self.block_header_subscription.poll_next_unpin(cx)) {
                None => {
                    self.finished = true;
                    return Poll::Ready(None)
                }
                Some(Err(e)) => {
                    self.finished = true;
                    return Poll::Ready(Some(Err(e.into())))
                }
                Some(Ok(block_header)) => {
                    // Note [jsdw]: We may be able to get rid of the per-item allocation
                    // with https://github.com/oblique/reusable-box-future.
                    self.at = Some(Box::pin(at(self.client, block_header.hash())));
                    // Continue, so that we poll this function future we've just created.
                }
            }
        }

        // If we get here, there will be an `at` function stored. Unwrap it and poll it to
        // completion to get our events, throwing it away as soon as it is ready.
        let at_fn = self
            .at
            .as_mut()
            .expect("'at' function should have been set above'");
        let events = futures::ready!(at_fn.poll_unpin(cx));
        self.at = None;
        Poll::Ready(Some(events))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Ensure `EventSubscription` can be sent; only actually a compile-time check.
    #[allow(unused)]
    fn check_sendability() {
        fn assert_send<T: Send>() {}
        assert_send::<
            EventSubscription<
                EventSub<<crate::DefaultConfig as Config>::Header>,
                crate::DefaultConfig,
                (),
            >,
        >();
        assert_send::<
            EventSubscription<
                FinalizedEventSub<<crate::DefaultConfig as Config>::Header>,
                crate::DefaultConfig,
                (),
            >,
        >();
    }
}