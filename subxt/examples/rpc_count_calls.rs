//! Wrap an RPC client to count calls per method, so we can see how many
//! `chainHead_v1_unpin` requests are made compared to the blocks we receive.
//!
//! Usage: `cargo run -p subxt --example rpc_count_calls --features reconnecting-rpc-client -- [url] [seconds]`
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use subxt::backend::ChainHeadBackend;
use subxt::{Error, OnlineClient, PolkadotConfig};
use subxt_rpcs::client::{
    RawRpcFuture, RawRpcSubscription, RawValue, ReconnectingRpcClient, RpcClient, RpcClientT,
};

type Counts = Arc<Mutex<BTreeMap<String, usize>>>;

/// Counts every request and subscription made through the inner client.
struct CountingClient<C> {
    inner: C,
    counts: Counts,
}

impl<C> CountingClient<C> {
    fn bump(&self, method: &str) {
        *self
            .counts
            .lock()
            .unwrap()
            .entry(method.to_owned())
            .or_default() += 1;
    }
}

impl<C: RpcClientT> RpcClientT for CountingClient<C> {
    fn request_raw<'a>(
        &'a self,
        method: &'a str,
        params: Option<Box<RawValue>>,
    ) -> RawRpcFuture<'a, Box<RawValue>> {
        self.bump(method);
        self.inner.request_raw(method, params)
    }

    fn subscribe_raw<'a>(
        &'a self,
        sub: &'a str,
        params: Option<Box<RawValue>>,
        unsub: &'a str,
    ) -> RawRpcFuture<'a, RawRpcSubscription> {
        self.bump(sub);
        self.inner.subscribe_raw(sub, params, unsub)
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "wss://asset-hub-kusama.rpc.turboflakes.io".to_string());
    let run_secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);

    let counts = Counts::default();
    let inner = ReconnectingRpcClient::builder()
        .build(&url)
        .await
        .map_err(Error::other)?;
    let rpc_client = RpcClient::new(CountingClient {
        inner,
        counts: counts.clone(),
    });

    // Use the chainHead backend directly so we know unpin calls are in play.
    let backend =
        ChainHeadBackend::<PolkadotConfig>::builder().build_with_background_driver(rpc_client);
    let api = OnlineClient::<PolkadotConfig>::from_backend(Arc::new(backend)).await?;

    let mut blocks = api.stream_blocks().await?;
    let mut finalized_seen = 0usize;
    let deadline = tokio::time::sleep(Duration::from_secs(run_secs));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            block = blocks.next() => {
                let Some(block) = block else { break };
                let block = block?;
                finalized_seen += 1;
                println!("Finalized block #{} ({finalized_seen} seen)", block.number());
            }
        }
    }

    println!("\nFinalized blocks seen: {finalized_seen}");
    println!("RPC calls over {run_secs}s:");
    for (method, count) in counts.lock().unwrap().iter() {
        println!("  {method}: {count}");
    }
    Ok(())
}
