extern crate dotenv;

use std::{ sync::Arc, thread, time::Duration };

use anyhow::{ Ok, Result };
use env::{ get_env, init_env, AppType };
use futures::future::join_all;
use queue::base_job::PubSubMessage;
use sui_data_ingestion_core::setup_single_workflow;
use scan_worker::IndexerWorker;
use uuid::Uuid;
use worker::setup_worker_flow;

pub mod library;
pub mod transaction;
pub mod utils;
pub mod env;
pub mod scan_worker;
pub mod queue;
pub mod worker;

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
      let (executor, _term_sender) = setup_single_workflow(
        worker,
        "https://checkpoints.mainnet.sui.io".to_string(),
        initital_checkpoint /* initial checkpoint number */,
        env_var.concurrency /* concurrency */,
        None /* extra reader options */
      ).await?;

      executor.await?;
      performance_task.abort();
    }
    AppType::Worker => {
      setup_worker_flow(env_var.concurrency as u8).await?;
    }
    AppType::Test => {
      let client = Arc::new(reqwest::Client::new());
      thread::sleep(Duration::from_secs(3));
      for _ in 0..5_000_0 {
        join_all(
          vec![0; 20].iter().map(|_| {
            let client_clone = client.clone();

            tokio::spawn(async move { client_clone
                .post("http://0.0.0.0:2811/message")
                .json(
                  &(PubSubMessage {
                    id: Uuid::new_v4().to_string(),
                    key: "data".to_string(),
                  })
                )
                .send().await })
          })
        ).await;
      }
    }
  }

  Ok(())
}
