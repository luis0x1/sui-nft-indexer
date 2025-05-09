use std::{ collections::BTreeMap, sync::Arc, thread, time::{ Duration, SystemTime } };

use anyhow::{ Ok, Result };
use async_trait::async_trait;
use sui_data_ingestion_core::Worker;
use sui_types::full_checkpoint_content::CheckpointData;
use tokio::{ sync::{ Mutex, MutexGuard }, task::JoinHandle };
use tokio_postgres::{ Client, NoTls };
use reqwest::Client as HttpClient;
use crate::{
  library::{
    object_wrapper::DataDefWrapper,
    sui_client::SuiClientProvider,
  },
  queue::{
    base_job::{ BaseJob, MessageContent },
    save_object_job::{ SaveObjectsJob, SaveObjectsJobPayload },
  },
  transaction::get_all_object_checkpoint,
};

pub struct AppState {
  pub datatypes: BTreeMap<String, DataDefWrapper>,
}

pub struct AppProvider {
  pub pg_client: Arc<Client>,
  pub sui_client: Arc<SuiClientProvider>,
  pub redis_client: Arc<redis::Client>,
  pub http_client: Arc<HttpClient>,
  pub state: Arc<Mutex<AppState>>,
}

impl AppProvider {
  pub async fn init() -> Result<Self> {
    let sui_client = SuiClientProvider::init().await?;
    let (client, connection) = tokio_postgres::connect(
      "postgresql://postgres:dai@localhost:5432/mydb",
      NoTls
    ).await?;

    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("connection error: {}", e);
      }
    });

    let redis_client = redis::Client::open("redis://127.0.0.1/9")?;

    Ok(AppProvider {
      pg_client: Arc::new(client),
      sui_client: Arc::new(sui_client),
      redis_client: Arc::new(redis_client),
      http_client: Arc::new(HttpClient::builder().build()?),
      state: Arc::new(Mutex::new(AppState { datatypes: BTreeMap::new() })),
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

  pub async fn get_dataref(&self, key: &str) -> Option<DataDefWrapper> {
    let datatypes = MutexGuard::map(self.state.lock().await, |state| &mut state.datatypes);

    if datatypes.contains_key(key) {
      return Some(datatypes[key].clone());
    }

    None
  }

  pub async fn set_dataref(&self, key: &str, value: DataDefWrapper) -> Result<()> {
    let mut datatypes = MutexGuard::map(self.state.lock().await, |state| &mut state.datatypes);

    (*datatypes).insert(key.to_string(), value);

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
static mut LAST_CHECKED: u64 = 135210073;
static mut CURRENT_CHECKPOINT: u64 = 135210073;
// static mut LAST_CHECKED: u64 = 0;
// static mut CURRENT_CHECKPOINT: u64 = 0;

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
      &MessageContent::SaveObjects(SaveObjectsJobPayload {
        objects: transaction_objects
          .iter()
          .map(|object| serde_json::to_string(object).unwrap())
          .collect(),
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

  Ok(())
}

impl IndexerWorker {
  pub fn provider(&self) -> &AppProvider {
    &self.0
  }
}

impl IndexerWorker {
  pub async fn init() -> Result<(Self, u64, JoinHandle<()>)> {
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
    let worker = Self(AppProvider::init().await?);

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
