use anyhow::Result;
use openssl::ssl::{ SslConnector, SslMethod };
use postgres_openssl::MakeTlsConnector;
use tokio_postgres::{ Client, NoTls };

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
    let builder = SslConnector::builder(SslMethod::tls())?;
    let connector = MakeTlsConnector::new(builder.build());
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
