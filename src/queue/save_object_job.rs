use std::{ str::FromStr, sync::Arc };

use crate::{
  library::{ display::StoredDisplay, package_resolve::PackageResolver, struct_resolver::StructResolver },
  scan_worker::AppProvider,
  transaction::{ SuiObject, TransactionObject },
  utils::object::{ get_object_content_bytes, insert_objects, is_valid_object },
};

use super::base_job::{ BaseJob, MessageContent };
use anyhow::{ Error, Result };
use async_trait::async_trait;
use move_core_types::language_storage::StructTag;
use serde::{ Deserialize, Serialize };
use sui_sdk::json::MoveTypeLayout;
use sui_types::{
  digests::TransactionDigest,
  move_package::MovePackage,
  object::{ Data, Object, Owner },
  TypeTag,
};

pub struct SaveObjectsJob;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveObjectsJobPayload {
  pub objects: String,
}

#[async_trait]
impl BaseJob<SaveObjectsJobPayload> for SaveObjectsJob {
  async fn handle(provider: &AppProvider, job: SaveObjectsJobPayload) -> anyhow::Result<()> {
    println!("doing job");
    let tx_objects: Vec<TransactionObject> = serde_json::from_str(&job.objects)?;
    let mut objects = Vec::<SuiObject>::new();
    let mut packages = Vec::<(MovePackage, Option<TransactionDigest>)>::new();
    let mut displays = Vec::<(StoredDisplay, Option<TransactionDigest>)>::new();

    for tx_object in tx_objects {
      match tx_object {
        TransactionObject::Struct(object, detail) => {
          if let Data::Move(move_object) = object.data.clone() {
            let object_type = object.type_().map(|v| v.to_string());

            if let Some(ref type_str) = object_type {
              if !is_valid_object(type_str.clone()) {
                continue;
              }
            }
            let move_struct_type = &move_object.type_().to_string();
            let struct_tag_res: Result<StructTag, _> = StructTag::from_str(&move_struct_type);
            let mut content = None;
            if let Ok(struct_tag) = struct_tag_res {
              let res = StructResolver::resolve_type_layout(
                provider,
                &TypeTag::Struct(Box::new(struct_tag)),
                20
              ).await;

              match res {
                Ok(data) => {
                  match data.0 {
                    MoveTypeLayout::Struct(layout) => {
                      let data = object.data.try_as_move().unwrap().to_move_struct(layout.as_ref());
                      match data {
                        Ok(d) => {
                          content = Some(serde_json::to_string(&d).unwrap());
                        }
                        _ => {}
                      }
                    }
                    _ => {}
                  };
                }
                Err(e) => {
                  eprintln!(
                    "error ================> {:?} -> {:?} -> {:?}",
                    object.id(),
                    object.type_().map(|t| t.to_string()),
                    e
                  );
                }
              }
            }
            let content_bytes = get_object_content_bytes(&object.data);

            objects.push(SuiObject {
              id: object.id().to_string(),
              owner: get_object_owner_address(&object),
              object_type,
              status: detail.status,
              updated_at: detail.confirmed_timestamp,
              content_bytes,
              content,
              version: object.version().to_string(),
            });
          }
        }
        TransactionObject::Package(package, detail) => {
          packages.push((package, Some(detail.digest)));
        }
        TransactionObject::Display(display, detail) => {
          displays.push((display, Some(detail.digest)));
        }
      }
    }

    insert_objects(provider, objects).await?;
    PackageResolver::save_packages(provider, packages).await?;
    PackageResolver::save_displays(provider, displays).await?;

    Ok(())
  }

  async fn dispatch(provider: &AppProvider, payload: MessageContent) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    match payload {
      MessageContent::SaveObjects(_) => {
        SaveObjectsJob::send(&mut connection, client, payload).await?;
      }
      _ => {
        return Err(Error::msg("Invalid SaveObjectJob payload"));
      }
    }

    Ok(())
  }
}

fn get_object_owner_address(object: &Object) -> String {
  match object.owner() {
    Owner::AddressOwner(addr) => addr.to_string(),
    Owner::ObjectOwner(addr) => addr.to_string(),
    Owner::Immutable => "Immutable".to_string(),
    Owner::Shared { initial_shared_version } => format!("Shared({})", initial_shared_version),
    Owner::ConsensusV2 { start_version: _, authenticator } => {
      authenticator.as_single_owner().to_string()
    }
  }
}
