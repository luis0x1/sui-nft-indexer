extern crate dotenv;

use anyhow::{ Ok, Result };
use env::{ get_env, init_env, AppType };
use library::blacklist_resolver::BlacklistResolver;
use sui_data_ingestion_core::setup_single_workflow;
use scan_worker::{AppProvider, IndexerWorker};
use worker::setup_worker_flow;

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
      let provider = AppProvider::init().await?;
      let res = BlacklistResolver::set_blacklist(&provider, "object_type", true).await;

      println!("res: {:?}", res);
    }
  }

  Ok(())
}
