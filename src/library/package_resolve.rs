use std::{ str::FromStr, sync::Arc };

use anyhow::{ anyhow, Error, Result };
use move_core_types::annotated_value::MoveStruct;
use serde::{ Deserialize, Serialize };
use sui_package_resolver::Package;
use sui_sdk::rpc_types::{
  SuiObjectData,
  SuiObjectDataOptions,
  SuiObjectResponse,
  SuiParsedData,
  SuiRawData,
  SuiTransactionBlockResponseOptions,
};
use sui_types::{
  base_types::ObjectID,
  collection_types::VecMap,
  digests::TransactionDigest,
  id::UID,
  move_package::MovePackage,
};
use crate::{ scan_worker::AppProvider, utils::error::OBJECT_NOT_FOUND_LOCAL };

use super::{ display::{ StoredDisplay }, sui_client::SuiClientProvider };

pub struct PackageResolver {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisplayObject {
  pub id: UID,
  pub fields: VecMap<String, String>,
  pub version: u16,
}

pub enum PackagePublishObject {
  UpgradeCap(ObjectID /* new package id */),
  Display(StoredDisplay, Option<TransactionDigest>),
}

impl PackageResolver {
  pub async fn get_package(
    provider: &AppProvider,
    package_id: &str,
    only_local: bool
  ) -> Result<(Package, u64)> {
    let pg_client = provider.pg_client();

    let package_raw_res = pg_client.query_one(
      "SELECT serialized, version FROM packages WHERE id = $1 OR virtual_id = $1 ORDER BY version DESC",
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

    if only_local {
      return Err(anyhow!(OBJECT_NOT_FOUND_LOCAL));
    }

    let package = PackageResolver::get_package_from_blockchain(provider, package_id).await?;
    Ok(package)
  }

  async fn get_package_from_blockchain(
    provider: &AppProvider,
    package_id_raw: &str
  ) -> Result<(Package, u64)> {
    let sui_client = provider.sui_client();
    let mut displays: Vec<(StoredDisplay, Option<TransactionDigest>)> = Vec::new();
    let package_id = ObjectID::from_str(package_id_raw)?;

    let package_with_tx_opt = sui_client.open_client(|client| async move {
      Ok(
        client
          .read_api()
          .get_object_with_options(
            package_id,
            SuiObjectDataOptions::new().with_previous_transaction()
          ).await?
      )
    }).await?;

    let Some(package_with_tx) = package_with_tx_opt.data else {
      return Err(Error::msg("PACKAGE_NOT_FOUND"));
    };

    let mut pkg_id = package_id;

    if let Some(previous_tx) = package_with_tx.previous_transaction {
      if let Ok(package_objects) = Self::get_deploy_tx_detail(&sui_client, previous_tx).await {
        // pkg_id = new_package_id;
        package_objects.iter().for_each(|package_object| {
          match package_object {
            PackagePublishObject::UpgradeCap(new_package_id) => {
              pkg_id = new_package_id.clone();
            }
            PackagePublishObject::Display(display_stored, transaction) => {
              displays.push((display_stored.clone(), transaction.clone()));
            }
            // _ => {}
          }
        });
      }
    }

    let object_data = sui_client.open_client(|client| async move {
      Ok(
        client
          .read_api()
          .get_object_with_options(
            pkg_id,
            SuiObjectDataOptions::new().with_bcs().with_previous_transaction()
          ).await?
      )
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
    if displays.len() > 0 {
      let save_object_res = Self::save_displays(provider, displays).await;
      match save_object_res {
        Ok(_) => println!("Save display successfully"),
        Err(_) => println!("Save display failed"),
      }
    }

    Self::save_package(
      provider,
      &move_package,
      Some(package_id),
      package_data.previous_transaction
    ).await?;
    let package = Package::read_from_package(&move_package)?;

    return Ok((package, move_package.version().value()));
  }

  pub async fn save_package(
    provider: &AppProvider,
    package: &MovePackage,
    virtual_id: Option<ObjectID>,
    previous_transaction: Option<TransactionDigest>
  ) -> Result<()> {
    let pg_client = provider.pg_client();

    let package_raw = serde_json::to_string(package)?;
    let tx = match previous_transaction {
      Some(ref t) => &t.to_string(),
      None => "",
    };

    let package_id = package.id().to_string();
    pg_client.query(
      "INSERT INTO packages(id, virtual_id, serialized, version, last_tx_digist, updated_at, created_at)
          VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
      ON CONFLICT(id)
      DO UPDATE SET serialized = $3, version = $4, last_tx_digist = $5",
      &[
        &package_id,
        &virtual_id.unwrap_or(package.id()).to_string(),
        &package_raw,
        &(u64::from(package.version()) as i64),
        &(if tx == "" { Some(tx) } else { None }),
      ]
    ).await?;

    Ok(())
  }

  pub async fn save_packages(
    provider: &AppProvider,
    packages: Vec<(MovePackage, Option<TransactionDigest>)>
  ) -> Result<()> {
    let pg_client = provider.pg_client();

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
          let virtual_id = package.original_package_id();

          format!(
            r#"('{}', '{}', $${}$$, '{}', {}, NOW(), NOW())"#,
            package.id(),
            virtual_id,
            serializer,
            u64::from(package.version()) as i64,
            tx
          )
        })
        .collect::<Vec<String>>();

      pg_client.query(
        &format!(
          "INSERT INTO packages(id, virtual_id, serialized, version, last_tx_digist, updated_at, created_at)
        VALUES {}
        ON CONFLICT (id)
        DO UPDATE 
          SET serialized = EXCLUDED.serialized",
          values.join(",")
        ),
        &[]
      ).await?;
    }

    Ok(())
  }

  pub async fn get_display(
    provider: &AppProvider,
    move_struct_type: &str,
    move_struct: &MoveStruct
  ) -> Result<String> {
    let mut display: Option<String> = None;
    let display_template_opt = Self::get_stored_display(provider, move_struct_type).await?;
    if let Some(display_template) = display_template_opt {
      let display_fields_res = StoredDisplay::get_rendered_fields(
        &display_template.fields,
        move_struct
      );

      if let Ok(display_fields) = display_fields_res {
        display = display_fields.data.map(|d| serde_json::to_string(&d).unwrap());
      }
    }

    if display.is_none() {
      return Err(anyhow!("display is null"));
    }

    Ok(display.unwrap())
  }

  pub async fn get_stored_display(
    provider: &AppProvider,
    move_struct_type: &str
  ) -> Result<Option<StoredDisplay>> {
    let mut display: Option<StoredDisplay> = None;
    let display_template_opt = provider.get_display(move_struct_type).await;
    if let Some(display_template) = display_template_opt {
      display = Some(display_template.as_ref().clone());
    }

    if display.is_some() {
      return Ok(display);
    }

    let pg_client = provider.pg_client();

    let display_raw_res = pg_client.query_one(
      "SELECT fields::TEXT, version FROM displays WHERE object_type = $1 ORDER BY version DESC",
      &[&move_struct_type]
    ).await;

    if let Ok(display_raw_db) = display_raw_res {
      let display_raw: String = display_raw_db.get("fields");
      let display_opt: Result<StoredDisplay, _> = serde_json::from_str(&display_raw);

      if let Ok(d) = display_opt {
        display = Some(d);
      }
    }

    Ok(display)
  }

  pub async fn save_displays(
    provider: &AppProvider,
    displays: Vec<(StoredDisplay, Option<TransactionDigest>)>
  ) -> Result<()> {
    let pg_client = provider.pg_client();

    let display_values: Vec<(String, Arc<StoredDisplay>)> = displays
      .iter()
      .map(|d| {
        let display = d.0.clone();
        (display.object_type.to_string(), Arc::new(display))
      })
      .collect();

    const BATCH_SIZE: usize = 1000;
    for chunk in displays.chunks(BATCH_SIZE) {
      let values = chunk
        .iter()
        .map(|(display, digest)| {
          let tx = match digest {
            Some(ref t) => &format!("'{}'", t.to_string()),
            None => "NULL",
          };
          let fields = serde_json::to_string(display).unwrap();

          format!(
            r#"('{}', '{}'::text, $${}$$::jsonb, '{}', {}, NOW(), NOW())"#,
            display.object_type,
            "[]",
            fields,
            display.version,
            tx
          )
        })
        .collect::<Vec<String>>();

      pg_client.query(
        &format!(
          "INSERT INTO displays(object_type, bcs, fields, version, last_tx_digist, updated_at, created_at)
        VALUES {}
        ON CONFLICT (object_type)
        DO UPDATE 
          SET fields = EXCLUDED.fields,
            version = EXCLUDED.version
        WHERE displays.version < EXCLUDED.version",
          values.join(",")
        ),
        &[]
      ).await?;

      provider.set_displays(display_values.clone()).await;
    }

    Ok(())
  }

  async fn get_deploy_tx_detail(
    sui_client: &SuiClientProvider,
    previous_tx: TransactionDigest
  ) -> Result<Vec<PackagePublishObject>> {
    let mut package_objects = Vec::<PackagePublishObject>::new();
    let latest_tx_res = sui_client.open_client(|client| async move {
      Ok(
        client
          .read_api()
          .get_transaction_with_options(
            previous_tx,
            SuiTransactionBlockResponseOptions::new().with_object_changes()
          ).await?
      )
    }).await;

    if let Ok(latest_tx) = latest_tx_res {
      if let Some(object_changes) = latest_tx.object_changes {
        let object_ids = &Arc::new(
          object_changes
            .iter()
            .map(|obj| obj.object_id())
            .collect::<Vec<ObjectID>>()
        );

        let objects_res = Self::get_all_objects(sui_client, object_ids).await;

        if let Ok(objects) = objects_res {
          objects.iter().for_each(|object| {
            if let Some(data) = &object.data {
              if let Some(type_id) = data.type_.as_ref() {
                let type_id_str = type_id.to_string();

                if let Some(package_object) = Self::handle_upgraded_cap(&type_id_str, data) {
                  package_objects.push(package_object);
                }

                if let Some(package_object) = Self::handle_display(&type_id_str, data) {
                  package_objects.push(package_object);
                }
              }
            }
          });
        }
      }
    }

    Ok(package_objects)
  }

  async fn get_all_objects(
    sui_client: &SuiClientProvider,
    object_ids: &Vec<ObjectID>
  ) -> Result<Vec<SuiObjectResponse>> {
    let mut objects = Vec::<SuiObjectResponse>::with_capacity(object_ids.len());
    for chunk in object_ids.chunks(50) {
      let objects_res = sui_client.open_client(|client| async move {
        Ok(
          client
            .read_api()
            .multi_get_object_with_options(
              chunk.to_vec(),
              SuiObjectDataOptions::new().with_content().with_type()
            ).await?
        )
      }).await?;

      objects.extend(objects_res);
    }

    Ok(objects)
  }

  fn handle_upgraded_cap(type_id: &String, data: &SuiObjectData) -> Option<PackagePublishObject> {
    if type_id.ends_with("2::package::UpgradeCap") {
      if let Some(content) = &data.content {
        if let SuiParsedData::MoveObject(object_data) = content {
          let new_pkg_id = object_data.fields.field_value("package");

          if new_pkg_id.is_some() {
            let new_object_id = ObjectID::from_str(&new_pkg_id.unwrap().to_string());

            if new_object_id.is_ok() {
              return Some(PackagePublishObject::UpgradeCap(new_object_id.unwrap()));
            }
          }
        }
      }
    }

    None
  }

  fn handle_display(type_id: &String, data: &SuiObjectData) -> Option<PackagePublishObject> {
    if type_id.contains("2::display::Display") {
      if let Some(content) = &data.content {
        if let SuiParsedData::MoveObject(object_data) = content {
          let fields = object_data.fields.clone().to_json_value();
          let display_object_res: Result<DisplayObject, _> = serde_json::from_value(fields);
          if let Ok(display_object) = display_object_res {
            return Some(
              PackagePublishObject::Display(
                StoredDisplay::try_from_display_object(&type_id, &display_object)?,
                data.previous_transaction
              )
            );
          }
        }
      }
    }

    None
  }
}
