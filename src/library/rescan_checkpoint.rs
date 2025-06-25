use anyhow::Result;
use async_scoped::TokioScope;
use futures::StreamExt;
use reqwest::Client as HttpClient;
use std::{
	sync::{Arc, LazyLock},
	thread,
	time::Duration,
};
use sui_types::full_checkpoint_content::CheckpointData;
use tap::Pipe;
use tokio::sync::mpsc;

use crate::{scan_worker::IndexerWorker, utils::unsafe_mutex::UnsafeMutex};

static BATCH_SIZE: LazyLock<Arc<UnsafeMutex<u64>>> =
	LazyLock::new(|| Arc::new(UnsafeMutex::new(10000)));
const CHANNEL_SIZE: usize = 10_000;

pub async fn setup_rescan_flow(
	worker: IndexerWorker,
	initial_checkpoint: u64,
	concurency: usize,
) -> Result<()> {
	let mut http_clients = Vec::new();

	for _ in 0..concurency {
		http_clients.push(Arc::new(HttpClient::builder().build()?));
	}

	TokioScope::scope_and_block(|scope| {
		for worker_index in 0..concurency {
			let http_client = Arc::clone(&http_clients[worker_index]);
			let worker_arc = Arc::new(&worker);
			let (sender, mut receiver) = mpsc::channel::<(u64, Option<CheckpointData>)>(CHANNEL_SIZE);

			scope.spawn(async move {
				while let Some((checkpoint_seq, checkpoint_data)) = receiver.recv().await {
					let _ = worker_arc
						.process_checkpoint(checkpoint_seq, checkpoint_data)
						.await;
				}
			});

			scope.spawn(async move {
				let mut scan_index = 0;
				let batch_size = BATCH_SIZE.read().as_ref().clone();
				let start_checkpoint = initial_checkpoint + batch_size * (worker_index as u64);
				loop {
					let start_cp = start_checkpoint + scan_index * batch_size * (concurency as u64);
					let end_cp = start_cp + batch_size;

					let mut checkpoint_stream = (start_cp..end_cp)
						.map(|checkpoint_number| take_checkpoint(&http_client, checkpoint_number as u64))
						.pipe(futures::stream::iter)
						.buffered(100);

					while let Some(checkpoint) = checkpoint_stream.next().await {
						let _ = sender.send(checkpoint).await;
					}
					// let current_checkpoint = take_checkpoint(&latest_checkpoint_arc).await;

					scan_index += 1;
					thread::sleep(Duration::from_secs(1));
				}
			});
		}
	});

	Ok(())
}

pub async fn take_checkpoint(
	http_client: &Arc<HttpClient>,
	checkpoint_num: u64,
) -> (u64, Option<CheckpointData>) {
	let checkpoint_res = get_checkpoint(http_client, checkpoint_num).await;

	let Ok(checkpoint_data) = checkpoint_res else {
		println!("checkpoint_res: {:?}", checkpoint_res.err());
		return (checkpoint_num, None);
	};

	(checkpoint_num, Some(checkpoint_data))
}

async fn get_checkpoint(
	http_client: &Arc<HttpClient>,
	checkpoint_num: u64,
) -> Result<CheckpointData> {
	let res = http_client
		.get(format!(
			"https://checkpoints.mainnet.sui.io/{}.chk",
			checkpoint_num
		))
		.send()
		.await?;

	println!("res: {:?} {:?}", res.version(), res.headers());

	let bytes = res.bytes().await?;

	let checkpoint: CheckpointData = bcs::from_bytes(&bytes[1..])?;
	Ok(checkpoint)
}
