use std::{sync::Arc, thread, time::Duration};

use crate::{
	env::get_env,
	library::{
		display::StoredDisplay, object_wrapper::DataDefWrapper, pg::get_postgres_connection,
		sui_client::SuiClientProvider,
	},
	utils::btree_map::BTreeMapLimit,
};
use anyhow::Result;
use deadpool_postgres::{Object, Pool};
use redis::AsyncCommands;
use reqwest::Client as HttpClient;
use sui_types::full_checkpoint_content::CheckpointData;
use tokio::{
	sync::{Mutex, MutexGuard},
	task::JoinHandle,
};

pub struct AppState {
	pub datatypes: BTreeMapLimit<String, Arc<DataDefWrapper>>,
	pub displays: BTreeMapLimit<String, Arc<StoredDisplay>>,
	pub blacklist_structs: BTreeMapLimit<String, bool>,
}

pub struct AppProvider {
	pub pg_client: Arc<Pool>,
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
			state: Arc::new(Mutex::new(AppState {
				datatypes: BTreeMapLimit::new(1000_000),
				displays: BTreeMapLimit::new(1000_000),
				blacklist_structs: BTreeMapLimit::new(1000_000),
			})),
		})
	}

	pub async fn pg_client(&self) -> Result<Object> {
		let pg_client = self.pg_client.get().await?;
		Ok(pg_client)
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

	pub async fn is_blacklist(&self, key: &str) -> Option<bool> {
		let blacklist_structs = MutexGuard::map(self.state.lock().await, |state| {
			&mut state.blacklist_structs
		});

		if blacklist_structs.contains_key(key) {
			return Some(*blacklist_structs.get(key).unwrap());
		}

		None
	}

	pub async fn set_blacklist(&self, key: &str, is_blacklist: bool) -> Option<bool> {
		let mut blacklist_structs = MutexGuard::map(self.state.lock().await, |state| {
			&mut state.blacklist_structs
		});

		if blacklist_structs.contains_key(key) {
			return Some(*blacklist_structs.get(key).unwrap());
		}

		(*blacklist_structs).insert(key.to_string(), is_blacklist);

		None
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
		display_values: Vec<(String, Arc<StoredDisplay>)>,
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

impl IndexerWorker {
	pub fn provider(&self) -> &AppProvider {
		&self.0
	}
}

impl IndexerWorker {
	pub async fn init() -> Result<(Self, u64, JoinHandle<()>)> {
		let provider = AppProvider::init().await?;
		let env_var = get_env();

		let mut checkpoint_conn = provider
			.redis_client
			.get_multiplexed_tokio_connection()
			.await?;
		let mut has_latest_checkpoint = false;

		if env_var.use_latest_checkpoint {
			let current_checkpoint_res = provider
				.sui_client
				.open_client(|client| async move {
					Ok(
						client
							.read_api()
							.get_latest_checkpoint_sequence_number()
							.await?,
					)
				})
				.await;

			if let Ok(current_checkpoint) = current_checkpoint_res {
				unsafe {
					CURRENT_CHECKPOINT = current_checkpoint;
					LAST_CHECKED = current_checkpoint;
				}
			}
		} else if env_var.start_checkpoint.is_some() {
			let value = env_var.start_checkpoint.value();
			unsafe {
				has_latest_checkpoint = true;
				CURRENT_CHECKPOINT = value;
				LAST_CHECKED = value;
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
			let current_checkpoint_res = provider
				.sui_client
				.open_client(|client| async move {
					Ok(
						client
							.read_api()
							.get_latest_checkpoint_sequence_number()
							.await?,
					)
				})
				.await;

			if let Ok(current_checkpoint) = current_checkpoint_res {
				unsafe {
					CURRENT_CHECKPOINT = current_checkpoint;
					LAST_CHECKED = current_checkpoint;
				}
			}
		}

		let initital_checkpoint = unsafe { CURRENT_CHECKPOINT };
		let performance_task = tokio::spawn(async {
			let mut cancel_process = 0;
			loop {
				thread::sleep(Duration::from_secs(10));
				unsafe {
					// if cancel_process >= 2 {
					//   // kill process if cannot fetch checkpoint
					//   std::process::exit(1);
					// }

					let range = CURRENT_CHECKPOINT - LAST_CHECKED;
					if range <= 0 {
						cancel_process += 1;
					} else {
						cancel_process = 0;
					}
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

impl IndexerWorker {
	pub async fn process_checkpoint(
		&self,
		checkpoint_sequence: u64,
		data: Option<CheckpointData>,
	) -> Result<()> {
		// let provider = self.provider();
		// if let Some(checkpoint) = data {
		//   process_checkpoint_clone(provider, &checkpoint.clone()).await?;
		// } else {
		//   HandleFailedCheckpointJob(checkpoint_sequence).dispatch(provider).await?;
		// }
		println!(
			"handle checkpoint {} successfully {:?}",
			checkpoint_sequence,
			data.map(|cp| cp.all_objects().len()),
		);
		Ok(())
	}
}
