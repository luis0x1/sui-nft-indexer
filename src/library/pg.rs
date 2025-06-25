use anyhow::Result;
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod};
use deadpool_redis::Runtime;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::NoTls;

use crate::env::get_env;

pub async fn get_postgres_connection(disable_ssl: bool) -> Result<Pool> {
	let env_var = get_env();
	let pool: Pool;
	let mut cfg = Config::new();
	cfg.url = Some(env_var.postgres_url);
	cfg.manager = Some(ManagerConfig {
		recycling_method: RecyclingMethod::Fast,
	});

	if disable_ssl {
		pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
	} else {
		let builder = TlsConnector::builder()
			.danger_accept_invalid_certs(true)
			.build()?;
		let connector = MakeTlsConnector::new(builder);
		pool = cfg.create_pool(Some(Runtime::Tokio1), connector).unwrap();
	}

	Ok(pool)
}
