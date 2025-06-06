use std::sync::Arc;
use futures::StreamExt;
use reqwest::Client as HttpClient;
use async_scoped::TokioScope;
use anyhow::{ anyhow, Result };
use sui_types::full_checkpoint_content::CheckpointData;
use tap::Pipe;
use tokio::sync::{ mpsc::{ self, Sender } };

use crate::{ scan_worker::IndexerWorker, utils::retry::{ retry_with_options, RetryOptions } };

static mut BATCH_SIZE: u64 = 100;
const CHANNEL_SIZE: usize = 10_000;

pub async fn setup_scan_flow(
  worker: IndexerWorker,
  initial_checkpoint: u64,
  concurency: usize
) -> Result<()> {
  let http_client = Arc::new(HttpClient::builder().build()?);
  let mut worker_channels: Vec<Arc<Sender<(u64, Option<CheckpointData>)>>> = Vec::new();

  TokioScope::scope_and_block(|scope| {
    for _ in 0..concurency {
      let worker_arc = Arc::new(&worker);
      let (sender, mut receiver) = mpsc::channel::<(u64, Option<CheckpointData>)>(CHANNEL_SIZE);
      worker_channels.push(Arc::new(sender));

      scope.spawn(async move {
        while let Some((checkpoint_seq, checkpoint_data)) = receiver.recv().await {
          let _ = worker_arc.process_checkpoint(checkpoint_seq, checkpoint_data).await;
        }
      });
      println!("run here");
    }

    scope.spawn(async move {
      let mut scan_index = 0;
      let mut worker_index = 0;
      loop {
        let batch_size;
        unsafe {
          batch_size = BATCH_SIZE;
        }

        let start_cp = initial_checkpoint + scan_index * batch_size;
        let end_cp = start_cp + batch_size;

        let mut checkpoint_stream = (start_cp..end_cp)
          .map(|checkpoint_number| take_checkpoint(&http_client, checkpoint_number as u64))
          .pipe(futures::stream::iter)
          .buffered(10);

        while let Some(checkpoint) = checkpoint_stream.next().await {
          let sender = worker_channels[worker_index].clone();
          let _ = sender.send(checkpoint).await;
          worker_index += 1;
          if worker_index >= concurency {
            worker_index = 0;
          }
        }
        // let current_checkpoint = take_checkpoint(&latest_checkpoint_arc).await;

        scan_index += 1;
      }
    });
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
  let bytes = retry_with_options(
    || async {
      let res = http_client
        .get(format!("https://checkpoints.mainnet.sui.io/{}.chk", checkpoint_num))
        .send().await?;
      let status = res.status().as_u16();
      if status >= 300 {
        if status == 404 {
          unsafe {
            BATCH_SIZE = 5;
          }
        }
        return Err(anyhow!("{:?}", res.status()));
      }
      let bytes = res.bytes().await?;

      Ok(bytes)
    },
    RetryOptions::default()
      .with_count(u64::MAX as u8)
      .with_delay(200)
  ).await?;
  let checkpoint: CheckpointData = bcs::from_bytes(&bytes[1..])?;
  Ok(checkpoint)
}
