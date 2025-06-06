extern crate dotenv;

use anyhow::{ Ok, Result };
use env::{ get_env, init_env, AppType };
use library::checkpoint::setup_scan_flow;
use scan_worker::IndexerWorker;
use worker::setup_worker_flow;

use crate::{
  library::rescan_checkpoint::setup_rescan_flow,
  queue::{ base_job::BaseJob, handle_failed_checkpoint_job::{ HandleFailedCheckpointJob } },
  scan_worker::AppProvider,
};

pub mod library;
pub mod transaction;
pub mod utils;
pub mod env;
pub mod scan_worker;
pub mod queue;
pub mod worker;
pub mod constants;

#[tokio::main]
async fn main() -> Result<()> {
  init_env();
  println!("ENV: {:?}", get_env());
  let env_var = get_env();
  // The connection object performs the actual communication with the database,
  // so spawn it off to run on its own.
  // println!("tempfile::tempdir()?.into_path(): {:?}", tempfile::tempdir()?.into_path());

  match env_var.app_type {
    AppType::Scan => {
      let (worker, initital_checkpoint, performance_task) = IndexerWorker::init().await?;
      println!("init done");
      setup_scan_flow(
        worker,
        initital_checkpoint /* initial checkpoint number */,
        env_var.concurrency /* concurrency */
      ).await?;

      performance_task.abort();
    }
    AppType::ReScan => {
      let (worker, initital_checkpoint, performance_task) = IndexerWorker::init().await?;
      println!("init done: {}", initital_checkpoint);
      setup_rescan_flow(
        worker,
        initital_checkpoint /* initial checkpoint number */,
        env_var.concurrency /* concurrency */
      ).await?;

      performance_task.abort();
    }
    AppType::Worker => {
      setup_worker_flow(env_var.concurrency as u8).await?;
    }
    AppType::Test => {
      let provider = &AppProvider::init().await?;
      HandleFailedCheckpointJob::handle(provider, HandleFailedCheckpointJob(151129784)).await?;
    }
  }

  Ok(())
}

// async fn get_checkpoint(http_client: Arc<reqwest::Client>, checkpoint_seq: u64) {
//   let start = SystemTime::now();
//   let bytes = http_client
//     .get(format!("https://checkpoints.mainnet.sui.io/{}.chk", checkpoint_seq))
//     .send().await
//     .unwrap()
//     .bytes().await
//     .unwrap();
//   let done_api = SystemTime::now();
//   let checkpoint: Result<CheckpointData, _> = bcs::from_bytes(&bytes[1..]);
//   let end = SystemTime::now();
//   println!(
//     "res: {:?} -> {:?} -> {:?}",
//     checkpoint.map(|cp| cp.checkpoint_summary.sequence_number),
//     done_api.duration_since(start),
//     end.duration_since(done_api)
//   );
// }
