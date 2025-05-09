use std::str::FromStr;

use anyhow::{ Error, Result };
use redis::AsyncCommands;
use sui_package_resolver::Package;
use sui_sdk::rpc_types::{ SuiObjectDataOptions, SuiRawData };
use sui_types::{ base_types::ObjectID, digests::TransactionDigest, move_package::MovePackage };

use crate::scan_worker::AppProvider;

use super::display::StoredDisplay;

pub struct PackageResolver {}

impl PackageResolver {
  pub async fn get_package(provider: &AppProvider, package_id: &str) -> Result<(Package, u64)> {
    let pg_client = provider.pg_client();
    let redis_client = provider.redis_client();

    let redis_key = format!("package_{}", package_id);
    let package_raw_res: Result<String, _> = redis_client
      .get_multiplexed_tokio_connection().await?
      .get(&redis_key).await;

    if let Ok(package_raw) = package_raw_res {
      let move_package_opt: Result<(MovePackage, u64), _> = serde_json::from_str(&package_raw);

      if let Ok(move_package) = move_package_opt {
        let package_res = Package::read_from_package(&move_package.0);

        if let Ok(package) = package_res {
          return Ok((package, move_package.1));
        }
      }
    }

    let package_raw_res = pg_client.query_one(
      "SELECT serialized, version FROM packages WHERE id = $1",
      &[&package_id]
    ).await;

    if let Ok(move_package_res) = package_raw_res {
      let move_package_raw: String = move_package_res.get("serialized");
      let version: i64 = move_package_res.get("version");
      let move_package_opt: Result<MovePackage, _> = serde_json::from_str(&move_package_raw);

      if let Ok(move_package) = move_package_opt {
        let package_res = Package::read_from_package(&move_package);

        if let Ok(package) = package_res {
          return Ok((package, version as u64));
        }
      }
    }

    let package = PackageResolver::get_package_from_blockchain(provider, package_id).await?;
    Ok(package)
  }

  async fn get_package_from_blockchain(
    provider: &AppProvider,
    package_id_raw: &str
  ) -> Result<(Package, u64)> {
    let sui_client = provider.sui_client();

    let package_id = ObjectID::from_str(package_id_raw)?;
    let object_data = sui_client.open_client(|client| async move {
      client
        .read_api()
        .get_object_with_options(package_id, SuiObjectDataOptions::new().with_bcs()).await
    }).await?;

    let Some(package_data) = object_data.data else {
      return Err(Error::msg("PACKAGE_NOT_FOUND"));
    };

    let Some(raw_data) = package_data.bcs else {
      return Err(Error::msg("PACKAGE_BCS_EMPTY"));
    };

    let SuiRawData::Package(package_content) = raw_data else {
      return Err(Error::msg("OBJECT_IS_NOT_PACKAGE"));
    };
    let move_package = package_content.to_move_package(u64::MAX)?;
    Self::save_package(provider, &move_package, package_data.previous_transaction).await?;
    let package = Package::read_from_package(&move_package)?;

    return Ok((package, move_package.version().value()));
  }

  pub async fn save_package(
    provider: &AppProvider,
    package: &MovePackage,
    previous_transaction: Option<TransactionDigest>
  ) -> Result<()> {
    let pg_client = provider.pg_client();
    let redis_client = provider.redis_client();
    let mut connection = redis_client.get_multiplexed_tokio_connection().await?;

    let package_raw = serde_json::to_string(package)?;
    let tx = match previous_transaction {
      Some(ref t) => &t.to_string(),
      None => "",
    };

    let package_id = package.id().to_string();
    pg_client.query(
      "INSERT INTO packages(id, serialized, version, last_tx_digist, updated_at, created_at)
          VALUES ($1, $2, $3, $4, NOW(), NOW())",
      &[
        &package_id,
        &package_raw,
        &(u64::from(package.version()) as i64),
        &(if tx == "" { Some(tx) } else { None }),
      ]
    ).await?;

    let _: () = connection.set(&format!("package_{}", package_id), package_raw).await?;

    Ok(())
  }

  pub async fn save_packages(
    provider: &AppProvider,
    packages: Vec<(&MovePackage, &Option<TransactionDigest>)>
  ) -> Result<()> {
    let pg_client = provider.pg_client();
    let redis_client = provider.redis_client();
    let mut connection = redis_client.get_multiplexed_tokio_connection().await?;

    const BATCH_SIZE: usize = 1000;
    for chunk in packages.chunks(BATCH_SIZE) {
      let values = chunk
        .iter()
        .map(|(package, digest)| {
          let tx = match digest {
            Some(ref t) => &format!("'{}'", t.to_string()),
            None => "NULL",
          };
          let serializer = serde_json::to_string(package).unwrap();
          (
            format!("package_{}", package.id().to_string()),
            serializer.clone(),
            format!(
              r#"('{}', '{}', '{}', {}, NOW(), NOW())"#,
              package.id(),
              serializer,
              u64::from(package.version()) as i64,
              tx
            ),
          )
        })
        .collect::<Vec<(String, String, String)>>();

      pg_client.query(
        &format!(
          "INSERT INTO packages(id, serialized, version, last_tx_digist, updated_at, created_at)
        VALUES {}
        ON CONFLICT (id)
        DO UPDATE 
          SET serialized = EXCLUDED.serialized",
          values
            .iter()
            .map(|(_, __, v)| v.to_string())
            .collect::<Vec<String>>()
            .join(",")
        ),
        &[]
      ).await?;

      let mut pipe = redis::pipe();
      pipe.atomic();
      values.iter().for_each(|(id, value, _)| {
        pipe.set(id, value);
      });

      let _: () = pipe.query_async(&mut connection).await?;
    }

    Ok(())
  }

  pub async fn save_displays(
    provider: &AppProvider,
    displays: Vec<(&StoredDisplay, &Option<TransactionDigest>)>
  ) -> Result<()> {
    let pg_client = provider.pg_client();
    let redis_client = provider.redis_client();
    let mut connection = redis_client.get_multiplexed_tokio_connection().await?;

    const BATCH_SIZE: usize = 1000;
    for chunk in displays.chunks(BATCH_SIZE) {
      let values = chunk
        .iter()
        .map(|(display, digest)| {
          let tx = match digest {
            Some(ref t) => &format!("'{}'", t.to_string()),
            None => "NULL",
          };
          let fields = serde_json::to_string(&display.fields).unwrap();

          (
            format!("display_{}", display.object_type),
            fields.clone(),
            format!(
              r#"('{}', '{}'::text, '{}'::jsonb, '{}', {}, NOW(), NOW())"#,
              display.object_type,
              &format!("{:?}", display.bcs),
              fields,
              display.version,
              tx
            ),
          )
        })
        .collect::<Vec<(String, String, String)>>();

      pg_client.query(
        &format!(
          "INSERT INTO displays(object_type, bcs, fields, version, last_tx_digist, updated_at, created_at)
        VALUES {}
        ON CONFLICT (object_type)
        DO UPDATE 
          SET fields = EXCLUDED.fields,
            version = EXCLUDED.version
        WHERE displays.version < EXCLUDED.version",
          values
            .iter()
            .map(|(_, __, v)| v.to_string())
            .collect::<Vec<String>>()
            .join(",")
        ),
        &[]
      ).await?;

      let mut pipe = redis::pipe();
      pipe.atomic();
      values.iter().for_each(|(id, value, _)| {
        pipe.set(id, value);
      });

      let _: () = pipe.query_async(&mut connection).await?;
    }

    Ok(())
  }
}
