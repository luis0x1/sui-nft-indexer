use std::{str::FromStr, sync::LazyLock};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::{string, utils::none_zero_u64::NoneZeroU64};

#[derive(Debug, Clone)]
pub struct EnvValue {
	pub worm_nft_type: String,
	pub app_type: AppType,
	pub channel_id: String,
	pub postgres_url: String,
	pub redis_url: String,
	pub worker_url: String,
	pub use_latest_checkpoint: bool,
	pub start_checkpoint: NoneZeroU64,
	pub concurrency: usize,
	pub disable_ssl: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum AppType {
	Scan = 1,
	Worker = 2,
	Test = 3,
	ReScan = 4,
}

const ZERO_ADDRESS: &'static str =
	"0x0000000000000000000000000000000000000000000000000000000000000000";

pub const CLMM_PACKAGE: LazyLock<String> =
	LazyLock::new(|| load_env("CLMM_PACKAGE", string!(ZERO_ADDRESS)));

pub const INTEGRATE_PACKAGE: LazyLock<String> =
	LazyLock::new(|| load_env("INTEGRATE_PACKAGE", string!(ZERO_ADDRESS)));

impl EnvValue {
	pub fn new() -> Self {
		let _ = dotenv::dotenv();

		EnvValue {
			worm_nft_type: load_env("WORM_NFT_TYPE", string!("default")),
			app_type: load_env("APP_TYPE", AppType::Scan),
			channel_id: load_env("CHANNEL_ID", string!("channel_id")),
			redis_url: load_env("REDIS_URL", string!("")),
			postgres_url: load_env("DATABASE_URL", string!("")),
			worker_url: load_env("WORKER_URL", string!("http://127.0.0.1:2811")),
			use_latest_checkpoint: load_env("USE_LATEST_CHECKPOINT", false),
			concurrency: load_env("CONCURRENCY", 1),
			start_checkpoint: load_env("START_CHECKPOINT", NoneZeroU64::default()),
			disable_ssl: load_env("DISABLE_SSL", true),
		}
	}
}

impl FromStr for AppType {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self> {
		Ok(match s {
			"1" => AppType::Scan,
			"2" => AppType::Worker,
			"3" => AppType::Test,
			"4" => AppType::ReScan,
			_ => AppType::Scan,
		})
	}
}

const ENV: LazyLock<EnvValue> = LazyLock::new(|| EnvValue::new());

pub fn get_env() -> EnvValue {
	let env = ENV.clone();
	env
}

fn load_env<T>(key: &str, default: T) -> T
where
	T: FromStr + Clone,
{
	std::env::var(key)
		.map(|v| v.parse::<T>().unwrap_or(default.clone()))
		.unwrap_or(default)
}
