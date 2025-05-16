use std::{ sync::Arc, thread, time::{ Duration, SystemTime } };

use anyhow::Result;
use async_trait::async_trait;
use redis::AsyncCommands;
use sui_data_ingestion_core::Worker;
use sui_types::{ full_checkpoint_content::CheckpointData };
use tokio::{ sync::{ Mutex, MutexGuard }, task::JoinHandle };
use tokio_postgres::Client;
use reqwest::Client as HttpClient;
use crate::{
  env::get_env,
  library::{
    display::StoredDisplay, object_wrapper::DataDefWrapper, pg::get_postgres_connection, sui_client::SuiClientProvider
  },
  queue::{
    base_job::{ BaseJob, MessageContent },
    save_object_job::{ SaveObjectsJob, SaveObjectsJobPayload },
  },
  transaction::get_all_object_checkpoint,
  utils::btree_map::BTreeMapLimit,
};

pub struct AppState {
  pub datatypes: BTreeMapLimit<String, Arc<DataDefWrapper>>,
  pub displays: BTreeMapLimit<String, Arc<StoredDisplay>>,
}

pub struct AppProvider {
  pub pg_client: Arc<Client>,
  pub sui_client: Arc<SuiClientProvider>,
  pub redis_client: Arc<redis::Client>,
  pub worker_client: Arc<redis::Client>,
  pub http_client: Arc<HttpClient>,
  pub state: Arc<Mutex<AppState>>,
}

impl AppProvider {
  pub async fn init() -> Result<Self> {
    let env_var = get_env();
    let sui_client = SuiClientProvider::init().await?;

    let client = get_postgres_connection(env_var.disable_ssl).await?;

    let redis_client = redis::Client::open(format!("{}/1", env_var.redis_url))?;
    let worker_client = redis::Client::open(format!("{}/9", env_var.redis_url))?;

    Ok(AppProvider {
      pg_client: Arc::new(client),
      sui_client: Arc::new(sui_client),
      redis_client: Arc::new(redis_client),
      worker_client: Arc::new(worker_client),
      http_client: Arc::new(HttpClient::builder().build()?),
      state: Arc::new(
        Mutex::new(AppState {
          datatypes: BTreeMapLimit::new(1000_000),
          displays: BTreeMapLimit::new(1000_000),
        })
      ),
    })
  }

  pub fn pg_client(&self) -> &Client {
    &self.pg_client
  }

  pub fn sui_client(&self) -> &SuiClientProvider {
    &self.sui_client
  }

  pub fn redis_client(&self) -> &redis::Client {
    &self.redis_client
  }

  pub async fn get_dataref(&self, key: &str) -> Option<Arc<DataDefWrapper>> {
    let datatypes = MutexGuard::map(self.state.lock().await, |state| &mut state.datatypes);

    datatypes.get(key).map(|d| d.clone())
  }

  pub async fn get_display(&self, key: &str) -> Option<Arc<StoredDisplay>> {
    let displays = MutexGuard::map(self.state.lock().await, |state| &mut state.displays);

    displays.get(key).map(|d| d.clone())
  }

  pub async fn set_display(&self, key: &str, display: StoredDisplay) -> Option<Arc<StoredDisplay>> {
    let mut displays = MutexGuard::map(self.state.lock().await, |state| &mut state.displays);

    (*displays).insert(key.to_string(), Arc::new(display))
  }

  pub async fn set_displays(
    &self,
    display_values: Vec<(String, Arc<StoredDisplay>)>
  ) -> Option<Arc<StoredDisplay>> {
    let mut displays = MutexGuard::map(self.state.lock().await, |state| &mut state.displays);

    (*displays).insert_many(display_values);

    None
  }

  pub async fn set_dataref(&self, key: &str, value: DataDefWrapper) -> Result<()> {
    let mut datatypes = MutexGuard::map(self.state.lock().await, |state| &mut state.datatypes);

    (*datatypes).insert(key.to_string(), Arc::new(value));

    Ok(())
  }
}

pub struct IndexerWorker(AppProvider);

//OBJECT
// static mut LAST_CHECKED: u64 = 133028503;
// static mut CURRENT_CHECKPOINT: u64 = 133028503;
//PACKAGE
// static mut LAST_CHECKED: u64 = 133027390;
// static mut CURRENT_CHECKPOINT: u64 = 133027390;
//BIRD_NFT
// static mut LAST_CHECKED: u64 = 131459304;
// static mut CURRENT_CHECKPOINT: u64 = 131459304;
//PACKAGE_NFT
// static mut LAST_CHECKED: u64 = 144654153;
// static mut CURRENT_CHECKPOINT: u64 = 144654153;
static mut LAST_CHECKED: u64 = 0;
static mut CURRENT_CHECKPOINT: u64 = 0;

async fn process_checkpoint_clone(
  provider: &AppProvider,
  checkpoint: &CheckpointData
) -> Result<()> {
  let start = SystemTime::now();
  // custom processing logic
  // print out the checkpoint number
  let transaction_objects = get_all_object_checkpoint(
    provider,
    checkpoint.transactions.clone(),
    checkpoint.checkpoint_summary.timestamp_ms
  ).await;

  let stop1 = SystemTime::now();

  let object_len = transaction_objects.len();

  // if package_len > 0 {
  //   let res = PackageResolver::save_packages(provider, packages).await;

  //   if res.is_err() {
  //     eprintln!(
  //       "handle packages {} failed: {:?}",
  //       checkpoint.checkpoint_summary.sequence_number,
  //       res.unwrap_err()
  //     );
  //   } else {
  //     let stop = SystemTime::now();

  //     println!(
  //       "handle packages {} successfully {}: total packages - {}: filled package, cost: 1: {:?} - 2: {:?}",
  //       checkpoint.checkpoint_summary.sequence_number,
  //       checkpoint.all_objects().len(),
  //       object_len,
  //       stop1.duration_since(start),
  //       stop.duration_since(start)
  //     );
  //   }
  // } else {
  //   println!("handle packages {} nothing to do", checkpoint.checkpoint_summary.sequence_number);
  // }

  // if display_len > 0 {
  //   let res = PackageResolver::save_displays(provider, displays).await;

  //   if res.is_err() {
  //     eprintln!(
  //       "handle displays {} failed: {:?}",
  //       checkpoint.checkpoint_summary.sequence_number,
  //       res.unwrap_err()
  //     );
  //   } else {
  //     let stop = SystemTime::now();

  //     println!(
  //       "handle displays {} successfully {}: total displays - {}: filled display, cost: 1: {:?} - 2: {:?}",
  //       checkpoint.checkpoint_summary.sequence_number,
  //       checkpoint.all_objects().len(),
  //       object_len,
  //       stop1.duration_since(start),
  //       stop.duration_since(start)
  //     );
  //   }
  // } else {
  //   println!("handle displays {} nothing to do", checkpoint.checkpoint_summary.sequence_number);
  // }

  if object_len > 0 {
    let res = SaveObjectsJob::dispatch(
      &provider,
      MessageContent::SaveObjects(SaveObjectsJobPayload {
        objects: serde_json::to_string(&transaction_objects)?,
      })
    ).await;

    if res.is_err() {
      eprintln!(
        "handle checkpoint {} failed: {:?}",
        checkpoint.checkpoint_summary.sequence_number,
        res.unwrap_err()
      );
    } else {
      let stop = SystemTime::now();

      println!(
        "handle checkpoint {} successfully {}: total objects - {}: filled object, cost: 1: {:?} - 2: {:?}",
        checkpoint.checkpoint_summary.sequence_number,
        checkpoint.all_objects().len(),
        object_len,
        stop1.duration_since(start),
        stop.duration_since(start)
      );
    }
  } else {
    println!("handle checkpoint {} nothing to do", checkpoint.checkpoint_summary.sequence_number);
  }

  unsafe {
    CURRENT_CHECKPOINT = checkpoint.checkpoint_summary.sequence_number;
  }
  let sequence_number = checkpoint.checkpoint_summary.sequence_number;
  if sequence_number > 10 {
    let _: () = provider.redis_client
      .get_multiplexed_async_connection().await?
      .set("checkpoint", (sequence_number - 10).to_string()).await?;
  }

  Ok(())
}

impl IndexerWorker {
  pub fn provider(&self) -> &AppProvider {
    &self.0
  }
}

impl IndexerWorker {
  pub async fn init() -> Result<(Self, u64, JoinHandle<()>)> {
    let provider = AppProvider::init().await?;
    let env_var = get_env();

    let mut checkpoint_conn = provider.redis_client.get_multiplexed_tokio_connection().await?;
    let mut has_latest_checkpoint = false;

    if env_var.use_latest_checkpoint {
      let current_checkpoint_res = provider.sui_client.open_client(|client| async move {
        Ok(client.read_api().get_latest_checkpoint_sequence_number().await?)
      }).await;

      if let Ok(current_checkpoint) = current_checkpoint_res {
        unsafe {
          CURRENT_CHECKPOINT = current_checkpoint;
          LAST_CHECKED = current_checkpoint;
        }
      }
    } else {
      let current_checkpoint_res: Result<String, _> = checkpoint_conn.get("checkpoint").await;

      if let Ok(current_checkpoint) = current_checkpoint_res {
        let checkpoint_num_res = current_checkpoint.parse::<u64>();
        if checkpoint_num_res.is_ok() {
          let checkpoint_num = checkpoint_num_res.unwrap();
          unsafe {
            has_latest_checkpoint = true;
            CURRENT_CHECKPOINT = checkpoint_num;
            LAST_CHECKED = checkpoint_num;
          }
        }
      }
    }

    if !has_latest_checkpoint {
      let current_checkpoint_res = provider.sui_client.open_client(|client| async move {
        Ok(client.read_api().get_latest_checkpoint_sequence_number().await?)
      }).await;

      if let Ok(current_checkpoint) = current_checkpoint_res {
        unsafe {
          CURRENT_CHECKPOINT = current_checkpoint;
          LAST_CHECKED = current_checkpoint;
        }
      }
    }

    let initital_checkpoint = unsafe { CURRENT_CHECKPOINT };
    let performance_task = tokio::spawn(async {
      loop {
        thread::sleep(Duration::from_secs(10));
        unsafe {
          let range = CURRENT_CHECKPOINT - LAST_CHECKED;
          LAST_CHECKED = CURRENT_CHECKPOINT;
          println!("---------------------------------------------------------");
          println!("scaned {} checkpoint after 10s", range);
          println!("---------------------------------------------------------");
        }
      }
    });
    let worker = Self(provider);

    Ok((worker, initital_checkpoint, performance_task))
  }
}

#[async_trait]
impl Worker for IndexerWorker {
  type Result = ();
  async fn process_checkpoint(&self, checkpoint: &CheckpointData) -> Result<()> {
    let provider = self.provider();
    process_checkpoint_clone(provider, &checkpoint.clone()).await?;
    Ok(())
  }
}
