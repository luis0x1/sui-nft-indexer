use std::sync::{ Arc, LazyLock, Mutex };

use serde::{ Deserialize, Serialize };

#[derive(Clone, Debug)]
pub struct EnvValue {
  pub worm_nft_type: String,
  pub app_type: AppType,
  pub channel_id: String,
  pub postgres_url: String,
  pub redis_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppType {
  Scan = 1,
  Worker = 2,
  Test = 3,
}

impl From<String> for AppType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "1" => AppType::Scan,
      "2" => AppType::Worker,
      "3" => AppType::Test,
      _ => AppType::Scan
    }
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
    })
  )
});

pub fn get_env() -> EnvValue {
  let env = ENV.lock().unwrap().clone();
  env
}

pub fn init_env() {
  dotenv::dotenv().unwrap();
  let mut env = ENV.lock().unwrap();
  env.worm_nft_type = std::env::var("WORM_NFT_TYPE").unwrap_or("default".to_string());
  env.app_type = std::env::var("APP_TYPE").unwrap_or("1".to_string()).into();
  env.channel_id = std::env::var("CHANNEL_ID").unwrap_or("channel_id".to_string());
  env.channel_id = std::env::var("DATABASE_URL").unwrap_or("".to_string());
  env.channel_id = std::env::var("REDIS_URL").unwrap_or("".to_string());
}
