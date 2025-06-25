use anyhow::{Result, anyhow};
use reqwest::{Client as HttpClient, Version};
use std::{collections::HashMap, sync::Arc};
use sui_types::full_checkpoint_content::CheckpointData;
use tokio::sync::Semaphore;

use crate::utils::{
	retry::{RetryOptions, retry_with_options},
	unsafe_mutex::UnsafeMutex,
};

#[derive(Clone)]
pub enum CheckpointRequest {
	Pending(Arc<Semaphore>),
}

pub struct CheckpointReader {
	data: Arc<UnsafeMutex<HashMap<u64, CheckpointRequest>>>,
	http_client: Arc<HttpClient>,
}

static mut BATCH_SIZE: u64 = 100;

impl CheckpointReader {
	pub fn new() -> Result<Self> {
		Ok(Self {
			data: Arc::new(UnsafeMutex::new(HashMap::new())),
			http_client: Arc::new(HttpClient::builder().http2_prior_knowledge().build()?),
		})
	}

	pub async fn get_checkpoint(
		&self,
		checkpoint_seq: u64,
	) -> Result<(u64, Option<CheckpointData>)> {
		let mut current_locker: Option<Arc<Semaphore>> = None;

		while current_locker.is_none() {
			let reader_state = self.data.read();
			if let Some(request) = reader_state.get(&checkpoint_seq) {
				match request {
					CheckpointRequest::Pending(locker) => {
						current_locker = Some(locker.clone());
					}
				}
			} else {
				println!("create new lock {}", checkpoint_seq);
				let setter_state = self.data.write().await?;

				setter_state.update(|mut state| {
					state.insert(
						checkpoint_seq,
						CheckpointRequest::Pending(Arc::new(Semaphore::new(1))),
					);

					state
				});
			}
		}

		let locked = current_locker.as_ref().unwrap().acquire().await?;

		let (_, res) = take_checkpoint(&self.http_client, checkpoint_seq).await;

		let Some(checkpoint_data) = res else {
			return Ok((checkpoint_seq, None));
		};

		drop(locked);

		Ok((checkpoint_seq, Some(checkpoint_data)))
	}
}

async fn take_checkpoint(
	http_client: &Arc<HttpClient>,
	checkpoint_num: u64,
) -> (u64, Option<CheckpointData>) {
	let checkpoint_res = get_checkpoint_data(http_client, checkpoint_num).await;

	let Ok(checkpoint_data) = checkpoint_res else {
		return (checkpoint_num, None);
	};

	(checkpoint_num, Some(checkpoint_data))
}

async fn get_checkpoint_data(
	http_client: &Arc<HttpClient>,
	checkpoint_num: u64,
) -> Result<CheckpointData> {
	println!("start called checkpoint {}", checkpoint_num);
	let bytes = retry_with_options(
		|| async {
			let res = http_client
				.get(format!(
					"https://checkpoints.mainnet.sui.io/{}.chk",
					checkpoint_num
				))
				.version(Version::HTTP_2)
				.send()
				.await?;
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
			.with_delay(200),
	)
	.await?;

	let checkpoint: CheckpointData = bcs::from_bytes(&bytes[1..])?;
	Ok(checkpoint)
}
