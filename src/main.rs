extern crate dotenv;

use std::{future::IntoFuture, sync::Arc, time::SystemTime};

use anyhow::{Ok, Result};
use chrono::{DateTime, Utc};
use env::{AppType, get_env};
use futures::StreamExt;
use library::checkpoint::setup_scan_flow;
use scan_worker::IndexerWorker;
use sui_types::full_checkpoint_content::CheckpointData;
use tap::Pipe;
use worker::setup_worker_flow;

use crate::library::rescan_checkpoint::{setup_rescan_flow, take_checkpoint};

pub mod constants;
pub mod env;
pub mod library;
pub mod queue;
pub mod scan_worker;
pub mod sui_move;
pub mod transaction;
pub mod utils;
pub mod worker;
pub mod scaner;

#[tokio::main]
async fn main() -> Result<()> {
	println!("ENV: {:?}", get_env());
	let env_var = get_env();
	// The connection object performs the actual communication with the database,
	// so spawn it off to run on its own.
	// println!("tempfile::tempdir()?.into_path(): {:?}", tempfile::tempdir()?.into_path());

	match env_var.app_type {
		AppType::Scan => {
			let (worker, initital_checkpoint, performance_task) = IndexerWorker::init().await?;
			println!("init done");
			let res = setup_scan_flow(
				worker,
				initital_checkpoint, /* initial checkpoint number */
				env_var.concurrency, /* concurrency */
			)
			.await;
			println!("run done: {:?}", res);

			performance_task.abort();
		}
		AppType::ReScan => {
			let (worker, initital_checkpoint, performance_task) = IndexerWorker::init().await?;
			println!("init done: {}", initital_checkpoint);
			setup_rescan_flow(
				worker,
				initital_checkpoint, /* initial checkpoint number */
				env_var.concurrency, /* concurrency */
			)
			.await?;

			performance_task.abort();
		}
		AppType::Worker => {
			setup_worker_flow(env_var.concurrency as u8).await?;
		}
		AppType::Test => {
			let start = SystemTime::now();
			let datetime: DateTime<Utc> = start.clone().into();
			println!("start: {:?}", datetime.format("%d/%m/%Y %T").to_string());
			let http_client = Arc::new(reqwest::Client::builder().build()?);
			let cps = (160000108..160001108)
				.map(|checkpoint_number| take_checkpoint(&http_client, checkpoint_number as u64))
				.pipe(futures::stream::iter)
				.buffered(50);

			let data: Vec<(u64, Option<CheckpointData>)> = cps.collect().into_future().await;
			println!("fun: {:?}", data.len());
			println!("cost: {:?}", start.elapsed());
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
