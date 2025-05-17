use std::{ str::FromStr, sync::{ Arc, LazyLock, Mutex } };

use anyhow::{ Result, Error };
use serde::{ Deserialize, Serialize };

use crate::string;

#[derive(Clone, Debug)]
pub struct EnvValue {
  pub worm_nft_type: String,
  pub app_type: AppType,
  pub channel_id: String,
  pub postgres_url: String,
  pub redis_url: String,
  pub worker_url: String,
  pub use_latest_checkpoint: bool,
  pub concurrency: usize,
  pub disable_ssl: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AppType {
  Scan = 1,
  Worker = 2,
  Test = 3,
}

impl FromStr for AppType {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self> {
    Ok(match s {
      "1" => AppType::Scan,
      "2" => AppType::Worker,
      "3" => AppType::Test,
      _ => AppType::Scan,
    })
  }
}

static ENV: LazyLock<Arc<Mutex<EnvValue>>> = LazyLock::new(|| {
  Arc::new(
    Mutex::new(EnvValue {
      worm_nft_type: "".to_string(),
      app_type: AppType::Scan,
      channel_id: "".to_string(),
      redis_url: "".to_string(),
      postgres_url: "".to_string(),
      worker_url: "".to_string(),
      use_latest_checkpoint: false,
      concurrency: 1,
      disable_ssl: true,
    })
  )
});

pub fn get_env() -> EnvValue {
  let env = ENV.lock().unwrap().clone();
  env
}

pub fn init_env() {
  let _ = dotenv::dotenv();
  let mut env = ENV.lock().unwrap();

  env.worm_nft_type = load_env("WORM_NFT_TYPE", string!("default"));
  env.app_type = load_env("APP_TYPE", AppType::Scan);
  env.channel_id = load_env("CHANNEL_ID", string!("channel_id"));
  env.postgres_url = load_env("DATABASE_URL", string!(""));
  env.redis_url = load_env("REDIS_URL", string!(""));
  env.worker_url = load_env("WORKER_URL", string!("http://127.0.0.1:2811"));
  env.use_latest_checkpoint = load_env("USE_LATEST_CHECKPOINT", false);
  env.concurrency = load_env("CONCURRENCY", 1);
  env.disable_ssl = load_env("DISABLE_SSL", true);
}

fn load_env<T>(key: &str, default: T) -> T where T: FromStr + Clone {
  std::env
    ::var(key)
    .map(|v| v.parse::<T>().unwrap_or(default.clone()))
    .unwrap_or(default)
}
