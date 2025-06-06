use std::{ sync::Arc, thread, time::Duration };
use futures::StreamExt;
use reqwest::Client as HttpClient;
use async_scoped::TokioScope;
use anyhow::Result;
use sui_types::full_checkpoint_content::CheckpointData;
use tap::Pipe;
use tokio::sync::mpsc;

use crate::scan_worker::IndexerWorker;

const BATCH_SIZE: u64 = 10000;
const CHANNEL_SIZE: usize = 10_000;

pub async fn setup_rescan_flow(
  worker: IndexerWorker,
  initial_checkpoint: u64,
  concurency: usize
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
          let _ = worker_arc.process_checkpoint(checkpoint_seq, checkpoint_data).await;
        }
      });

      scope.spawn(async move {
        let mut scan_index = 0;
        let start_checkpoint = initial_checkpoint + BATCH_SIZE * (worker_index as u64);
        loop {
          let start_cp = start_checkpoint + scan_index * BATCH_SIZE * (concurency as u64);
          let end_cp = start_cp + BATCH_SIZE;

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

async fn take_checkpoint(
  http_client: &Arc<HttpClient>,
  checkpoint_num: u64
) -> (u64, Option<CheckpointData>) {
  let checkpoint_res = get_checkpoint(http_client, checkpoint_num).await;

  let Ok(checkpoint_data) = checkpoint_res else {
    return (checkpoint_num, None);
  };

  (checkpoint_num, Some(checkpoint_data))
}

async fn get_checkpoint(
  http_client: &Arc<HttpClient>,
  checkpoint_num: u64
) -> Result<CheckpointData> {
  let bytes = http_client
    .get(format!("https://checkpoints.mainnet.sui.io/{}.chk", checkpoint_num))
    .send().await?
    .bytes().await?;

  let checkpoint: CheckpointData = bcs::from_bytes(&bytes[1..])?;
  Ok(checkpoint)
}
