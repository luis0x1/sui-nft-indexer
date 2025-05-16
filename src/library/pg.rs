use anyhow::Result;
use tokio_postgres::{ Client, NoTls };
use native_tls::{ TlsConnector };
use postgres_native_tls::MakeTlsConnector;

use crate::env::get_env;

pub async fn get_postgres_connection(disable_ssl: bool) -> Result<Client> {
  let env_var = get_env();
  #[warn(unused_variables)]
  let client: Client;

  if disable_ssl {
    let (_client, connection) = tokio_postgres::connect(&env_var.postgres_url, NoTls).await?;
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("connection error: {}", e);
      }
    });

    client = _client;
  } else {
    // let mut builder = SslConnector::builder(SslMethod::tls())?;
    // builder.set_verify(SslVerifyMode::NONE);
    let builder = TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
    let connector = MakeTlsConnector::new(builder);
    let (_client, connection) = tokio_postgres::connect(&env_var.postgres_url, connector).await?;
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("connection error: {}", e);
      }
    });

    client = _client;
  }

  Ok(client)
}
